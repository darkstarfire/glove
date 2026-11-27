#!/usr/bin/env python3
"""Desenha as imagens dos aparelhos do "Sobre Este Computador" (notebook, computador de
mesa, tudo-em-um, mini PC) em PNG com fundo transparente, com o papel de parede do Glove
OS nas telas. Uso: python3 tools/about-devices.py  (grava em crates/about/assets/)."""

import math
import os
import sys

import cairo

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "crates", "about", "assets")
WALL = os.path.join(ROOT, "data", "wallpapers", "glove-os.png")
W, H = 640, 440


def rrect(c, x, y, w, h, r):
    r = min(r, w / 2, h / 2)
    c.new_sub_path()
    c.arc(x + w - r, y + r, r, -math.pi / 2, 0)
    c.arc(x + w - r, y + h - r, r, 0, math.pi / 2)
    c.arc(x + r, y + h - r, r, math.pi / 2, math.pi)
    c.arc(x + r, y + r, r, math.pi, 3 * math.pi / 2)
    c.close_path()


def lin(x0, y0, x1, y1, stops):
    g = cairo.LinearGradient(x0, y0, x1, y1)
    for s in stops:
        g.add_color_stop_rgba(*s)
    return g


def hexa(h, a=1.0):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)) + (a,)


def stop(t, h, a=1.0):
    return (t,) + hexa(h, a)


def shadow(c, cx, cy, rx, ry, alpha=0.35):
    """Sombra macia no chão (elipse com degradê radial)."""
    c.save()
    c.translate(cx, cy)
    c.scale(1, ry / rx)
    g = cairo.RadialGradient(0, 0, 0, 0, 0, rx)
    g.add_color_stop_rgba(0, 0, 0, 0, alpha)
    g.add_color_stop_rgba(0.55, 0, 0, 0, alpha * 0.45)
    g.add_color_stop_rgba(1, 0, 0, 0, 0)
    c.set_source(g)
    c.arc(0, 0, rx, 0, 2 * math.pi)
    c.fill()
    c.restore()


def screen(c, x, y, w, h, r=0):
    """A tela acesa: o papel de parede, um brilho do vidro e a borda interna escura."""
    wall = cairo.ImageSurface.create_from_png(WALL)
    c.save()
    rrect(c, x, y, w, h, r)
    c.clip()
    k = max(w / wall.get_width(), h / wall.get_height())
    c.save()
    c.translate(x + (w - wall.get_width() * k) / 2, y + (h - wall.get_height() * k) / 2)
    c.scale(k, k)
    c.set_source_surface(wall, 0, 0)
    c.get_source().set_filter(cairo.FILTER_BEST)
    c.paint()
    c.restore()
    # Reflexo do vidro: uma faixa diagonal clara, bem suave.
    c.move_to(x + w * 0.42, y)
    c.line_to(x + w * 0.78, y)
    c.line_to(x + w * 0.38, y + h)
    c.line_to(x + w * 0.02, y + h)
    c.close_path()
    c.set_source(lin(x, y, x + w, y + h, [stop(0, "#ffffff", 0.10), stop(1, "#ffffff", 0.02)]))
    c.fill()
    # Vinheta nas bordas (a tela não é chapada).
    c.set_source(lin(x, y, x, y + h, [stop(0, "#000000", 0.10), stop(0.15, "#000000", 0), stop(0.85, "#000000", 0), stop(1, "#000000", 0.14)]))
    c.paint()
    c.restore()


def bezel(c, x, y, w, h, r, edge=True):
    """Moldura preta de vidro, com a borda de alumínio por fora."""
    if edge:
        rrect(c, x - 2.5, y - 2.5, w + 5, h + 5, r + 2.5)
        c.set_source(lin(0, y, 0, y + h, [stop(0, "#d9dce1"), stop(0.5, "#a8adb4"), stop(1, "#80858c")]))
        c.fill()
    rrect(c, x, y, w, h, r)
    c.set_source(lin(0, y, 0, y + h, [stop(0, "#232427"), stop(1, "#0d0d0f")]))
    c.fill()
    # Brilho fino no alto do vidro.
    rrect(c, x + 1, y + 1, w - 2, h - 2, r - 1)
    c.set_source_rgba(1, 1, 1, 0.06)
    c.set_line_width(1)
    c.stroke()


def camera(c, cx, cy):
    c.arc(cx, cy, 2.6, 0, 2 * math.pi)
    c.set_source(lin(cx, cy - 3, cx, cy + 3, [stop(0, "#2c3340"), stop(1, "#0a0c10")]))
    c.fill()
    c.arc(cx - 0.8, cy - 0.8, 0.8, 0, 2 * math.pi)
    c.set_source_rgba(0.5, 0.6, 0.9, 0.5)
    c.fill()


def notebook(c):
    shadow(c, W / 2, 372, 300, 16, 0.42)
    # Tampa.
    lx, ly, lw, lh = 92, 28, 456, 300
    bezel(c, lx, ly, lw, lh, 14)
    screen(c, lx + 14, ly + 16, lw - 28, lh - 30, 3)
    camera(c, W / 2, ly + 8)
    # Dobradiça escura entre a tampa e a base.
    c.rectangle(lx + 30, ly + lh - 2, lw - 60, 6)
    c.set_source(lin(0, ly + lh - 2, 0, ly + lh + 4, [stop(0, "#2a2b2e"), stop(1, "#55585d")]))
    c.fill()
    # Base: o tampo (em perspectiva) e a borda da frente.
    top, front = ly + lh + 2, ly + lh + 22
    c.move_to(lx - 6, top)
    c.line_to(lx + lw + 6, top)
    c.line_to(lx + lw + 44, front)
    c.line_to(lx - 44, front)
    c.close_path()
    c.set_source(lin(0, top, 0, front, [stop(0, "#f2f3f5"), stop(0.6, "#d4d7dc"), stop(1, "#c3c7cd")]))
    c.fill_preserve()
    c.set_source_rgba(0, 0, 0, 0.12)
    c.set_line_width(0.8)
    c.stroke()
    # Borda da frente, arredondada nas pontas.
    rrect(c, lx - 46, front - 1, lw + 92, 13, 6)
    c.set_source(lin(0, front, 0, front + 12, [stop(0, "#c9cdd3"), stop(0.45, "#a3a8af"), stop(1, "#6f747b")]))
    c.fill()
    # Reflexo no fio da borda.
    c.move_to(lx - 40, front + 0.8)
    c.line_to(lx + lw + 40, front + 0.8)
    c.set_source_rgba(1, 1, 1, 0.75)
    c.set_line_width(1)
    c.stroke()
    # O vão para abrir a tampa, no meio da frente.
    rrect(c, W / 2 - 34, front - 1, 68, 6, 3)
    c.set_source(lin(0, front, 0, front + 6, [stop(0, "#8e939a"), stop(1, "#b7bbc1")]))
    c.fill()


def desktop(c):
    shadow(c, 262, 376, 170, 12, 0.38)
    shadow(c, 560, 378, 80, 10, 0.42)
    # Monitor.
    mx, my, mw, mh = 34, 40, 420, 248
    bezel(c, mx, my, mw, mh, 10)
    screen(c, mx + 9, my + 9, mw - 18, mh - 30, 2)
    # Queixo com o LED de ligado.
    c.arc(mx + mw / 2, my + mh - 11, 1.8, 0, 2 * math.pi)
    c.set_source_rgba(0.55, 0.85, 1.0, 0.9)
    c.fill()
    # Haste e pé de alumínio.
    nx = mx + mw / 2
    c.move_to(nx - 26, my + mh)
    c.line_to(nx + 26, my + mh)
    c.line_to(nx + 30, 352)
    c.line_to(nx - 30, 352)
    c.close_path()
    c.set_source(lin(nx - 30, 0, nx + 30, 0, [stop(0, "#8f949b"), stop(0.35, "#e3e6ea"), stop(0.7, "#c2c6cc"), stop(1, "#7d8288")]))
    c.fill()
    rrect(c, nx - 92, 350, 184, 14, 7)
    c.set_source(lin(0, 350, 0, 364, [stop(0, "#eef0f3"), stop(0.5, "#c5c9cf"), stop(1, "#8a8f96")]))
    c.fill()
    # Torre ao lado.
    tx, ty, tw, th = 500, 120, 118, 252
    rrect(c, tx, ty, tw, th, 8)
    c.set_source(lin(tx, 0, tx + tw, 0, [stop(0, "#3a3c41"), stop(0.12, "#5d6067"), stop(0.5, "#2f3135"), stop(1, "#1d1e21")]))
    c.fill()
    # Frente com brilho vertical e as grades.
    rrect(c, tx + 6, ty + 6, tw - 12, th - 12, 5)
    c.set_source(lin(tx, 0, tx + tw, 0, [stop(0, "#2a2c30"), stop(0.45, "#3f4248"), stop(1, "#232428")]))
    c.fill()
    for i in range(18):
        y = ty + 70 + i * 9
        rrect(c, tx + 22, y, tw - 44, 3, 1.5)
        c.set_source_rgba(0, 0, 0, 0.45)
        c.fill()
        c.move_to(tx + 23, y + 3.6)
        c.line_to(tx + tw - 23, y + 3.6)
        c.set_source_rgba(1, 1, 1, 0.07)
        c.set_line_width(0.8)
        c.stroke()
    # Botão de ligar com anel azul.
    bx, by = tx + tw / 2, ty + 34
    c.arc(bx, by, 9, 0, 2 * math.pi)
    c.set_source(lin(0, by - 9, 0, by + 9, [stop(0, "#5a5d63"), stop(1, "#26282b")]))
    c.fill()
    c.arc(bx, by, 9, 0, 2 * math.pi)
    c.set_source_rgba(0.35, 0.7, 1.0, 0.9)
    c.set_line_width(1.6)
    c.stroke()
    c.arc(bx, by, 3.6, -math.pi * 0.3, math.pi * 1.3)
    c.set_source_rgba(0.75, 0.9, 1, 0.9)
    c.set_line_width(1.4)
    c.stroke()
    # Pés da torre.
    for fx in (tx + 10, tx + tw - 30):
        rrect(c, fx, ty + th - 2, 20, 6, 2)
        c.set_source_rgba(0.08, 0.08, 0.09, 1)
        c.fill()


def allinone(c):
    shadow(c, W / 2, 380, 190, 13, 0.4)
    x, y, w, h = 82, 18, 476, 306
    # Corpo de alumínio (a tela fica na parte de cima, o queixo embaixo).
    rrect(c, x, y, w, h, 14)
    c.set_source(lin(0, y, 0, y + h, [stop(0, "#e9ebee"), stop(0.8, "#cdd1d6"), stop(1, "#b2b7bd")]))
    c.fill()
    bezel(c, x + 3, y + 3, w - 6, h - 58, 11, edge=False)
    screen(c, x + 14, y + 14, w - 28, h - 80, 2)
    camera(c, W / 2, y + 8.5)
    # Queixo com reflexo.
    c.move_to(x + 14, y + h - 52)
    c.line_to(x + w - 14, y + h - 52)
    c.set_source_rgba(1, 1, 1, 0.6)
    c.set_line_width(1)
    c.stroke()
    # Pé inclinado.
    nx = W / 2
    c.move_to(nx - 54, y + h)
    c.line_to(nx + 54, y + h)
    c.line_to(nx + 62, 360)
    c.line_to(nx - 62, 360)
    c.close_path()
    c.set_source(lin(nx - 62, 0, nx + 62, 0, [stop(0, "#8b9097"), stop(0.3, "#dfe2e6"), stop(0.7, "#c0c4ca"), stop(1, "#7c8187")]))
    c.fill()
    rrect(c, nx - 72, 357, 144, 10, 4)
    c.set_source(lin(0, 357, 0, 367, [stop(0, "#f0f2f4"), stop(1, "#8f949b")]))
    c.fill()


def mini(c):
    shadow(c, W / 2, 284, 196, 12, 0.45)
    # Caixa baixa em vista de três quartos: o tampo (claro) e a frente.
    x0, x1, top_y, back_y, depth = 150, 490, 230, 160, 52
    inset = 46
    c.move_to(x0 + inset, back_y)
    c.line_to(x1 - inset, back_y)
    c.line_to(x1, top_y)
    c.line_to(x0, top_y)
    c.close_path()
    c.set_source(lin(0, back_y, 0, top_y, [stop(0, "#d7dade"), stop(1, "#f4f5f7")]))
    c.fill()
    # Brilho e o logotipo discreto no tampo.
    c.save()
    c.translate(W / 2, (back_y + top_y) / 2)
    c.scale(1, 0.32)
    c.arc(0, 0, 26, 0, 2 * math.pi)
    c.set_source_rgba(0.55, 0.58, 0.62, 0.35)
    c.fill()
    c.restore()
    # Frente.
    rrect(c, x0, top_y - 4, x1 - x0, depth, 10)
    c.set_source(lin(0, top_y, 0, top_y + depth, [stop(0, "#c6cad0"), stop(0.5, "#a9aeb5"), stop(1, "#7f848b")]))
    c.fill()
    c.move_to(x0 + 8, top_y - 2)
    c.line_to(x1 - 8, top_y - 2)
    c.set_source_rgba(1, 1, 1, 0.8)
    c.set_line_width(1.2)
    c.stroke()
    # Portas na frente e o LED.
    for i, px in enumerate((x0 + 40, x0 + 72)):
        rrect(c, px, top_y + 16, 22, 9, 3)
        c.set_source_rgba(0.12, 0.12, 0.14, 1)
        c.fill()
    rrect(c, x0 + 104, top_y + 17, 10, 7, 2)
    c.set_source_rgba(0.12, 0.12, 0.14, 1)
    c.fill()
    c.arc(x1 - 34, top_y + 21, 3, 0, 2 * math.pi)
    c.set_source_rgba(0.45, 0.8, 1.0, 1)
    c.fill()
    c.arc(x1 - 34, top_y + 21, 6, 0, 2 * math.pi)
    c.set_source_rgba(0.45, 0.8, 1.0, 0.18)
    c.fill()


def main():
    os.makedirs(OUT, exist_ok=True)
    for name, fn in [("notebook", notebook), ("desktop", desktop), ("allinone", allinone), ("mini", mini)]:
        s = cairo.ImageSurface(cairo.FORMAT_ARGB32, W, H)
        c = cairo.Context(s)
        c.set_antialias(cairo.ANTIALIAS_BEST)
        fn(c)
        path = os.path.join(OUT, f"{name}.png")
        s.write_to_png(path)
        print(path)


if __name__ == "__main__":
    sys.exit(main())
