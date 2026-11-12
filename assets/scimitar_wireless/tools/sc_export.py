import cv2, json, math, os
import numpy as np
import subprocess

# ----------------------------------------------------------------------------
# geometry helpers
# ----------------------------------------------------------------------------
def silhouette(b, thr=6, sigma=4):
    m = (b > thr).astype(np.uint8) * 255
    m = cv2.morphologyEx(m, cv2.MORPH_OPEN, np.ones((3, 3), np.uint8))
    n, lab, st, _ = cv2.connectedComponentsWithStats(m, 8)
    idx = 1 + int(np.argmax(st[1:, cv2.CC_STAT_AREA]))
    sil = (lab == idx).astype(np.uint8) * 255
    sb = cv2.GaussianBlur(sil, (0, 0), sigma)
    _, sb = cv2.threshold(sb, 128, 255, cv2.THRESH_BINARY)
    c = max(cv2.findContours(sb, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)[0],
            key=cv2.contourArea)
    return cv2.approxPolyDP(c, 1.8, True).reshape(-1, 2)

def seams(b, minlen=40):
    Hh, Ww = b.shape
    sm = cv2.medianBlur(b, 5)
    bh = cv2.morphologyEx(sm, cv2.MORPH_BLACKHAT,
                          cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (19, 19)))
    _, g = cv2.threshold(bh, 7, 255, cv2.THRESH_BINARY)
    g = cv2.morphologyEx(g, cv2.MORPH_CLOSE, np.ones((3, 3), np.uint8))
    sk = cv2.ximgproc.thinning(g)
    skb = sk > 0
    k = np.array([[1, 1, 1], [1, 0, 1], [1, 1, 1]], np.uint8)
    deg = cv2.filter2D(skb.astype(np.uint8), -1, k)
    py, px = np.nonzero(skb)
    starts = [(y, x) for y, x in zip(py, px) if deg[y, x] == 1] + list(zip(py, px))
    vis = np.zeros_like(skb, bool)
    paths = []
    for sy, sx in starts:
        if vis[sy, sx]:
            continue
        p = [(sx, sy)]
        vis[sy, sx] = True
        cy, cx = sy, sx
        while True:
            nxt = None
            for dy, dx in ((-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)):
                ny, nx = cy + dy, cx + dx
                if 0 <= ny < Hh and 0 <= nx < Ww and skb[ny, nx] and not vis[ny, nx]:
                    nxt = (ny, nx)
                    break
            if nxt is None:
                break
            cy, cx = nxt
            vis[cy, cx] = True
            p.append((cx, cy))
        if len(p) >= minlen:
            paths.append(p)
    out = []
    for p in paths:
        ap = cv2.approxPolyDP(np.array(p, np.int32).reshape(-1, 1, 2), 2.2, False).reshape(-1, 2)
        if len(ap) >= 2:
            out.append(ap)
    return out

def split_exclude(paths, boxes):
    def inside(pt, box):
        x0, y0, x1, y1 = box
        return x0 <= pt[0] <= x1 and y0 <= pt[1] <= y1
    keep = []
    for p in paths:
        runs, cur = [], []
        for q in p:
            if any(inside(q, b) for b in boxes):
                if len(cur) >= 2:
                    runs.append(cur)
                cur = []
            else:
                cur.append(q)
        if len(cur) >= 2:
            runs.append(cur)
        for r in runs:
            if len(r) < 8:
                continue
            ap = cv2.approxPolyDP(np.array(r, np.int32).reshape(-1, 1, 2), 2.2, False).reshape(-1, 2)
            if len(ap) >= 2:
                keep.append(ap)
    return keep

def rounded_rect(x0, y0, x1, y1, r, n=5):
    pts = []
    for cx, cy, a0 in ((x1 - r, y0 + r, -90), (x1 - r, y1 - r, 0),
                       (x0 + r, y1 - r, 90), (x0 + r, y0 + r, 180)):
        for i in range(n + 1):
            a = math.radians(a0 + 90 * i / n)
            pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts

# ----------------------------------------------------------------------------
# views
# ----------------------------------------------------------------------------
def build_top():
    b = cv2.imread('sc_body_1.jpg', cv2.IMREAD_GRAYSCALE)
    cont = silhouette(b, thr=14)
    box_logo = (1600, 420, 1830, 640)
    box_wheel = (1020, 490, 1310, 630)
    box_pill = (1295, 515, 1340, 592)
    box_panel = (1330, 495, 1520, 625)
    sm = split_exclude(seams(b, 35), [box_logo, box_wheel, box_pill, box_panel])

    slot = [(1056, 556), (1070, 508), (1272, 508), (1288, 556), (1272, 606), (1070, 606)]
    wheel_rr = rounded_rect(1094, 520, 1284, 594, 34)
    kn = []
    cx, cy = (1094 + 1284) / 2, (520 + 594) / 2
    step, ms = 15.0, 4.0
    for j in range(-5, 6):
        for i in range(-10, 11):
            x = cx + i * step + (step / 2 if j % 2 else 0)
            y = cy + j * step * 0.82
            if 1094 + 18 < x < 1284 - 18 and 520 + 12 < y < 594 - 12:
                kn.append([(x, y - ms), (x + ms, y), (x, y + ms), (x - ms, y)])
    pill = rounded_rect(1304, 530, 1330, 580, 13)
    plate = [(1348, 506), (1498, 501), (1508, 558), (1494, 612), (1352, 616)]
    face = rounded_rect(1362, 517, 1487, 603, 14)

    body = [('poly', cont, 2.6, True)] + [('poly', s, 1.4, False) for s in sm]
    wheel = [('poly', np.array(slot), 1.8, True),
             ('poly', np.array(wheel_rr), 1.8, True)] + \
            [('poly', np.array(q), 0.9, True) for q in kn]
    buttons = [('poly', np.array(pill), 1.8, True),
               ('poly', np.array(plate), 1.8, True),
               ('poly', np.array(face), 1.8, True)]
    return 'top', body, wheel, buttons

def build_side():
    b = cv2.imread('sc_body_2.jpg', cv2.IMREAD_GRAYSCALE)
    cont = silhouette(b, thr=14)
    TL, TR, BR, BL = (1300, 502), (1642, 442), (1700, 662), (1348, 728)
    def bil(u, v):
        a, bb, c, d = (1 - u) * (1 - v), u * (1 - v), u * v, (1 - u) * v
        return (a * TL[0] + bb * TR[0] + c * BR[0] + d * BL[0],
                a * TL[1] + bb * TR[1] + c * BR[1] + d * BL[1])
    grid = {}
    for row in range(3):
        for col in range(4):
            u0, u1 = col / 4, (col + 1) / 4
            v0, v1 = row / 3, (row + 1) / 3
            iu0, iu1 = u0 + (u1 - u0) * 0.03, u1 - (u1 - u0) * 0.03
            iv0, iv1 = v0 + (v1 - v0) * 0.03, v1 - (v1 - v0) * 0.03
            grid[(col, row)] = [bil(iu0, iv0), bil(iu1, iv0), bil(iu1, iv1), bil(iu0, iv1)]
    btns = []
    for row in range(3):
        for col in range(4):
            q = grid[(col, row)]
            pts = [np.array(q[0]), np.array(q[1]), np.array(q[2]), np.array(q[3])]
            out = []
            for i in range(4):
                prev, cur, nxt = pts[i - 1], pts[i], pts[(i + 1) % 4]
                v1 = prev - cur; v2 = nxt - cur
                l1, l2 = np.linalg.norm(v1), np.linalg.norm(v2)
                v1 /= l1; v2 /= l2
                a = cur + v1 * min(8, l1 * 0.45)
                c_ = cur + v2 * min(8, l2 * 0.45)
                out.append(a)
                for t in (0.4, 0.6):
                    out.append((a * (1 - t) + cur * t) * (1 - t) + c_ * t)
                out.append(c_)
            btns.append(out)

    TEXT = (1000, 420, 1480, 800)
    def inside(pt, box):
        x0, y0, x1, y1 = box
        return x0 <= pt[0] <= x1 and y0 <= pt[1] <= y1
    def in_btn(pt):
        for q in btns:
            if cv2.pointPolygonTest(np.array(q, np.float32), (float(pt[0]), float(pt[1])), False) >= 0:
                return True
        return False
    sm = []
    for p in seams(b, 35):
        runs, cur = [], []
        for q in p:
            if in_btn(q) or inside(q, TEXT):
                if len(cur) >= 2:
                    runs.append(cur)
                cur = []
            else:
                cur.append(q)
        if len(cur) >= 2:
            runs.append(cur)
        for r in runs:
            if len(r) < 8:
                continue
            ap = cv2.approxPolyDP(np.array(r, np.int32).reshape(-1, 1, 2), 2.2, False).reshape(-1, 2)
            sm.append(ap)

    body = [('poly', cont, 2.6, True)] + [('poly', s, 1.4, False) for s in sm]
    buttons = [('poly', np.array(q), 1.9, True) for q in btns]
    return 'side', body, buttons, []

# ----------------------------------------------------------------------------
# export
# ----------------------------------------------------------------------------
def bounds(layer_groups):
    xs, ys = [], []
    for group in layer_groups:
        for kind, pts, w, closed in group:
            a = np.array(pts, float)
            xs += [a[:, 0].min(), a[:, 0].max()]
            ys += [a[:, 1].min(), a[:, 1].max()]
    return min(xs), min(ys), max(xs), max(ys)

def write_svg(path, groups, box, margin_frac=0.03):
    x0, y0, x1, y1 = box
    mx = (x1 - x0) * margin_frac
    my = (y1 - y0) * margin_frac
    vx, vy = x0 - mx, y0 - my
    vw, vh = (x1 - x0) + 2 * mx, (y1 - y0) + 2 * my
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vx:.2f} {vy:.2f} {vw:.2f} {vh:.2f}" '
         f'width="{vw:.2f}" height="{vh:.2f}">',
         '<g fill="none" stroke="#ffffff" stroke-linecap="round" stroke-linejoin="round">']
    for group in groups:
        for kind, pts, w, closed in group:
            d = ' '.join(f'{x:.2f},{y:.2f}' for x, y in pts)
            tag = 'polygon' if closed else 'polyline'
            o.append(f'<{tag} points="{d}" stroke-width="{w}"/>')
    o += ['</g>', '</svg>']
    open(path, 'w').write('\n'.join(o))
    return vw, vh

def raster(svg, png, height):
    subprocess.run(['rsvg-convert', '-h', str(height), svg, '-o', png], check=True)
    img = cv2.imread(png, cv2.IMREAD_UNCHANGED)
    if img.shape[2] == 3:
        img = cv2.cvtColor(img, cv2.COLOR_BGR2BGRA)
    # white RGB everywhere, keep alpha -> clean tinting / filtering
    img[:, :, 0] = 255
    img[:, :, 1] = 255
    img[:, :, 2] = 255
    cv2.imwrite(png, img)
    return img.shape[1], img.shape[0]

def export(view, body, wheel, buttons, outdir, height):
    os.makedirs(outdir, exist_ok=True)
    groups = {'body': [body], 'wheel': [wheel] if wheel else [], 'buttons': [buttons] if buttons else []}
    allg = [g for g in (body, wheel, buttons) if g]
    box = bounds(allg)
    sizes = {}
    for name, g in groups.items():
        if not g:
            continue
        svg = os.path.join(outdir, f'{view}_{name}.svg')
        png = os.path.join(outdir, f'{view}_{name}.png')
        write_svg(svg, g, box)
        sizes[name] = raster(svg, png, height)
    svg = os.path.join(outdir, f'{view}.svg')
    png = os.path.join(outdir, f'{view}.png')
    write_svg(svg, allg, box)
    sizes['all'] = raster(svg, png, height)
    # 2x versions
    for name in list(sizes):
        s2 = os.path.join(outdir, f'{view}{"" if name=="all" else "_"+name}_2x.png')
        src = png if name == 'all' else os.path.join(outdir, f'{view}_{name}.png')
        raster(src.replace('.png', '.svg'), s2, height * 2)
    print(view, 'bbox', [round(v) for v in box], 'sizes', sizes)

out = 'scimitar_layers'
v, body, wheel, buttons = build_top()
export(v, body, wheel, buttons, out, 547)
v, body, buttons, _ = build_side()
export(v, body, [], buttons, out, 390)
