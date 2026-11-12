import cv2, json, math, os
import numpy as np
import subprocess

# ---------------------------------------------------------------- helpers
def silhouette(b, thr=12, sigma=4):
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

def seam_paths(b, minlen=35):
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

def bounds(groups):
    xs, ys = [], []
    for g in groups:
        for kind, pts, w, closed in g:
            a = np.array(pts, float)
            xs += [a[:, 0].min(), a[:, 0].max()]
            ys += [a[:, 1].min(), a[:, 1].max()]
    return min(xs), min(ys), max(xs), max(ys)

def write_svg(path, groups, box, margin=0.03):
    x0, y0, x1, y1 = box
    mx, my = (x1 - x0) * margin, (y1 - y0) * margin
    vx, vy = x0 - mx, y0 - my
    vw, vh = (x1 - x0) + 2 * mx, (y1 - y0) + 2 * my
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vx:.2f} {vy:.2f} {vw:.2f} {vh:.2f}" width="{vw:.2f}" height="{vh:.2f}">',
         '<g fill="none" stroke="#ffffff" stroke-linecap="round" stroke-linejoin="round">']
    for g in groups:
        for kind, pts, w, closed in g:
            d = ' '.join(f'{x:.2f},{y:.2f}' for x, y in pts)
            tag = 'polygon' if closed else 'polyline'
            o.append(f'<{tag} points="{d}" stroke-width="{w}"/>')
    o += ['</g>', '</svg>']
    open(path, 'w').write('\n'.join(o))

def raster(svg, png, height):
    subprocess.run(['rsvg-convert', '-h', str(height), svg, '-o', png], check=True)
    img = cv2.imread(png, cv2.IMREAD_UNCHANGED)
    if img.shape[2] == 3:
        img = cv2.cvtColor(img, cv2.COLOR_BGR2BGRA)
    img[:, :, 0] = 255; img[:, :, 1] = 255; img[:, :, 2] = 255
    cv2.imwrite(png, img)

def export(view, layers, outdir, height):
    os.makedirs(outdir, exist_ok=True)
    allg = []
    for g in layers.values():
        allg += g
    box = bounds([allg])
    for name, g in layers.items():
        if not g:
            continue
        write_svg(os.path.join(outdir, f'{view}_{name}.svg'), [g], box)
        raster(os.path.join(outdir, f'{view}_{name}.svg'),
               os.path.join(outdir, f'{view}_{name}.png'), height)
        raster(os.path.join(outdir, f'{view}_{name}.svg'),
               os.path.join(outdir, f'{view}_{name}_2x.png'), height * 2)
    write_svg(os.path.join(outdir, f'{view}.svg'), [allg], box)
    raster(os.path.join(outdir, f'{view}.svg'), os.path.join(outdir, f'{view}.png'), height)
    raster(os.path.join(outdir, f'{view}.svg'), os.path.join(outdir, f'{view}_2x.png'), height * 2)
    print(view, 'layers', list(layers), 'bbox', [round(v) for v in box])

# ---------------------------------------------------------------- darkstar top
def darkstar_top():
    b = cv2.imread('body_1.jpg', cv2.IMREAD_GRAYSCALE)
    cont = silhouette(b, 12)
    seams = split_exclude(seam_paths(b, 35),
                          [(1600, 380, 1930, 640), (1075, 495, 1320, 600), (1325, 508, 1510, 592)])
    slot = [(1056, 556), (1070, 508), (1272, 508), (1288, 556), (1272, 606), (1070, 606)]
    wheel = rounded_rect(1094, 520, 1284, 594, 34)
    kn = []
    cx, cy = 1189, 557
    for j in range(-5, 6):
        for i in range(-10, 11):
            x = cx + i * 15 + (7.5 if j % 2 else 0)
            y = cy + j * 15 * 0.82
            if 1112 < x < 1266 and 532 < y < 582:
                kn.append([(x, y - 4), (x + 4, y), (x, y + 4), (x - 4, y)])
    plate = [(1096, 505), (1505, 505), (1522, 545), (1505, 593), (1096, 593)]
    btns = [rounded_rect(1345, 520, 1413, 578, 6), rounded_rect(1428, 520, 1496, 578, 6)]
    return {
        'body': [('poly', cont, 2.6, True)] + [('poly', s, 1.4, False) for s in seams] + [('poly', np.array(plate), 1.4, False)],
        'wheel': [('poly', np.array(slot), 1.8, True), ('poly', np.array(wheel), 1.8, True)] +
                 [('poly', np.array(q), 0.9, True) for q in kn],
        'buttons': [('poly', np.array(q), 1.9, True) for q in btns],
    }

# ---------------------------------------------------------------- darkstar side (from the tuned DXF)
def darkstar_side():
    def parse(path):
        txt = open(path, errors='ignore').read().splitlines()
        i = 0
        while i < len(txt) - 1:
            if txt[i].strip() == '2' and txt[i + 1].strip() == 'ENTITIES':
                i += 2; break
            i += 1
        ents = []; cur = None
        while i < len(txt) - 1:
            code = txt[i].strip(); val = txt[i + 1].rstrip(); i += 2
            if code == '0':
                if cur: ents.append(cur)
                if val == 'ENDSEC': break
                cur = {'type': val, 'vx': [], 'vy': [], 'x2': None, 'y2': None, 'layer': None, 'closed': False}
            elif cur is not None:
                if code == '8': cur['layer'] = val.strip()
                elif code == '10': cur['vx'].append(float(val))
                elif code == '20': cur['vy'].append(float(val))
                elif code == '11': cur['x2'] = float(val)
                elif code == '21': cur['y2'] = float(val)
                elif code == '70': cur['closed'] = (int(float(val)) & 1) == 1
        if cur: ents.append(cur)
        return ents

    def to_px(x, y):
        return ((x - 30.0) / 0.12 + 950.0, 790.0 - (y - 38.0) / 0.12)

    body, wheel, buttons, lights = [], [], [], []
    for e in parse('drawing_v2.dxf'):
        if e['layer'] == 'OUTLINE':
            pts = [to_px(x, y) for x, y in zip(e['vx'], e['vy'])]
            body.append(('poly', np.array(pts), 2.6, True))
        elif e['layer'] == 'BUTTON':
            pts = tuple((round(x, 1), round(y, 1)) for x, y in zip(e['vx'], e['vy']))
            if any(tuple((round(x, 1), round(y, 1)) for x, y in zip(b[1][:, 0], b[1][:, 1])) == pts
                   for b in []):
                continue
            buttons.append(('poly', np.array([to_px(x, y) for x, y in zip(e['vx'], e['vy'])]), 1.9, True))
        elif e['layer'] == 'DETAIL':
            if e['type'] == 'LINE':
                a = to_px(e['vx'][0], e['vy'][0]); c = to_px(e['x2'], e['y2'])
                wheel.append(('poly', np.array([a, c]), 0.9, False))
            else:
                pts = [to_px(x, y) for x, y in zip(e['vx'], e['vy'])]
                arr = np.array(pts)
                x0, y0 = arr.min(axis=0); x1, y1 = arr.max(axis=0)
                if e['closed'] and len(pts) == 4 and x1 - x0 < 110 and y1 - y0 < 50:
                    lights.append(('poly', arr, 1.6, True))
                elif len(pts) >= 20:
                    wheel.append(('poly', arr, 1.6, False))
                else:
                    body.append(('poly', arr, 1.4, False))

    # dedupe identical buttons (the DXF has one duplicate)
    seen, uniq = set(), []
    for b in buttons:
        k = tuple(np.round(b[1], 1).flatten())
        if k in seen:
            continue
        seen.add(k); uniq.append(b)
    return {'body': body, 'wheel': wheel, 'buttons': uniq, 'lights': lights}

# ---------------------------------------------------------------- wired scimitar top
def wired_top():
    b = cv2.imread('sc_body_1.jpg', cv2.IMREAD_GRAYSCALE)
    cont = silhouette(b, 14)
    seams = split_exclude(seam_paths(b, 35),
                          [(1600, 420, 1830, 640), (1020, 490, 1310, 630),
                           (1295, 515, 1340, 592), (1298, 585, 1338, 648), (1330, 495, 1520, 625)])
    slot = [(1056, 556), (1070, 508), (1272, 508), (1288, 556), (1272, 606), (1070, 606)]
    wheel = rounded_rect(1094, 520, 1284, 594, 34)
    kn = []
    cx, cy = 1189, 557
    for j in range(-5, 6):
        for i in range(-10, 11):
            x = cx + i * 15 + (7.5 if j % 2 else 0)
            y = cy + j * 15 * 0.82
            if 1112 < x < 1266 and 532 < y < 582:
                kn.append([(x, y - 4), (x + 4, y), (x, y + 4), (x - 4, y)])
    pill1 = rounded_rect(1304, 530, 1330, 580, 13)
    pill2 = rounded_rect(1304, 590, 1330, 640, 13)
    plate = [(1348, 506), (1498, 501), (1508, 558), (1494, 612), (1352, 616)]
    face = rounded_rect(1362, 517, 1487, 603, 14)
    return {
        'body': [('poly', cont, 2.6, True)] + [('poly', s, 1.4, False) for s in seams],
        'wheel': [('poly', np.array(slot), 1.8, True), ('poly', np.array(wheel), 1.8, True)] +
                 [('poly', np.array(q), 0.9, True) for q in kn],
        'buttons': [('poly', np.array(pill1), 1.8, True), ('poly', np.array(pill2), 1.8, True),
                    ('poly', np.array(plate), 1.8, True), ('poly', np.array(face), 1.8, True)],
    }

# ---------------------------------------------------------------- wired scimitar side
def wired_side():
    b = cv2.imread('se_body_3.jpg', cv2.IMREAD_GRAYSCALE)
    cont = silhouette(b, 6)
    sm = cv2.medianBlur(b, 5)
    bh = cv2.morphologyEx(sm, cv2.MORPH_BLACKHAT, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (19, 19)))
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
        p = [(sx, sy)]; vis[sy, sx] = True; cy, cx = sy, sx
        while True:
            nxt = None
            for dy, dx in ((-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)):
                ny, nx = cy + dy, cx + dx
                if 0 <= ny < b.shape[0] and 0 <= nx < b.shape[1] and skb[ny, nx] and not vis[ny, nx]:
                    nxt = (ny, nx); break
            if nxt is None:
                break
            cy, cx = nxt; vis[cy, cx] = True; p.append((cx, cy))
        if len(p) >= 70:
            paths.append(p)

    # 12 buttons (face segmentation)
    roi = b[420:780, 1180:1740]
    x0r, y0r = 1180, 420
    m = (roi > 60).astype(np.uint8) * 255
    n, lab, st, _ = cv2.connectedComponentsWithStats(m, 8)
    cent = []
    for i in range(1, n):
        a = st[i, cv2.CC_STAT_AREA]; w = st[i, cv2.CC_STAT_WIDTH]; h = st[i, cv2.CC_STAT_HEIGHT]
        if not (1500 < a < 20000 and 55 < w < 105 and 55 < h < 105):
            continue
        if st[i, cv2.CC_STAT_LEFT] + x0r > 1640:
            continue
        cent.append((st[i, 0] + w / 2 + x0r, st[i, 1] + h / 2 + y0r))
    cent.sort(key=lambda c: c[0])
    cols = [sorted(cent[i * 3:(i + 1) * 3], key=lambda c: c[1]) for i in range(4)]
    G = [[cols[c][r] for c in range(4)] for r in range(3)]
    right = np.mean([np.array(G[r][c + 1]) - np.array(G[r][c]) for r in range(3) for c in range(3)], axis=0)
    down = np.mean([np.array(G[r + 1][c]) - np.array(G[r][c]) for r in range(2) for c in range(4)], axis=0)

    def quad_at(c, ax=0.46, ay=0.46, rad=9):
        c = np.array(c, float)
        pts = [c - right * ax - down * ay, c + right * ax - down * ay,
               c + right * ax + down * ay, c - right * ax + down * ay]
        out = []
        for i in range(4):
            prev, cur, nxt = pts[i - 1], pts[i], pts[(i + 1) % 4]
            v1 = prev - cur; v2 = nxt - cur
            l1, l2 = np.linalg.norm(v1), np.linalg.norm(v2)
            v1 /= l1; v2 /= l2
            a = cur + v1 * min(rad, l1 * 0.45); c_ = cur + v2 * min(rad, l2 * 0.45)
            out.append(a)
            for t in (0.4, 0.6):
                out.append((a * (1 - t) + cur * t) * (1 - t) + c_ * t)
            out.append(c_)
        return out

    btns = [quad_at(G[r][c]) for r in range(3) for c in range(4)]

    def in_btn(pt):
        for q in btns:
            if cv2.pointPolygonTest(np.array(q, np.float32), (float(pt[0]), float(pt[1])), False) >= 0:
                return True
        return False

    TEXT = (1080, 360, 1330, 480)
    def inside(pt, box):
        x0, y0, x1, y1 = box
        return x0 <= pt[0] <= x1 and y0 <= pt[1] <= y1

    sm2 = []
    for p in paths:
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
            if len(r) < 10:
                continue
            ap = cv2.approxPolyDP(np.array(r, np.int32).reshape(-1, 1, 2), 2.2, False).reshape(-1, 2)
            sm2.append(ap)

    return {
        'body': [('poly', cont, 2.6, True)] + [('poly', s, 1.4, False) for s in sm2],
        'buttons': [('poly', np.array(q), 1.9, True) for q in btns],
    }

out = 'mice_layers'
export('top', darkstar_top(), os.path.join(out, 'darkstar'), 547)
export('side', darkstar_side(), os.path.join(out, 'darkstar'), 390)
export('top', wired_top(), os.path.join(out, 'scimitar_wired'), 547)
export('side', wired_side(), os.path.join(out, 'scimitar_wired'), 390)
