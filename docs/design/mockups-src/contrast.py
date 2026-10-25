def h2rgb(h):
    h=h.lstrip('#'); return [int(h[i:i+2],16)/255 for i in (0,2,4)]
def lum(c):
    def f(v): return v/12.92 if v<=0.03928 else ((v+0.055)/1.055)**2.4
    r,g,b=[f(v) for v in c]; return 0.2126*r+0.7152*g+0.0722*b
def blend(fg,bg,a): return [a*x+(1-a)*y for x,y in zip(fg,bg)]
def cr(a,b):
    la,lb=lum(a),lum(b); la,lb=max(la,lb),min(la,lb); return (la+0.05)/(lb+0.05)
themes={
 'sample-omarchy':dict(background='#1f2120',lighter_background='#2a2c2a',foreground='#e4dccb',muted='#9c9585',accent='#d9a35b',red='#e07a6e',yellow='#d9b45b',green='#98bf7c',sel=0.18,fill=0.04,border=0.40),
 'default-dark':dict(background='#14171b',lighter_background='#1d2126',foreground='#e7eaee',muted='#9aa3ae',accent='#4fc3b4',red='#f08a80',yellow='#e6c15c',green='#7fd48f',sel=0.18,fill=0.04,border=0.40),
 'default-light':dict(background='#f5f6f8',lighter_background='#ffffff',foreground='#1a1d21',muted='#5a626c',accent='#0b7a6e',red='#b42318',yellow='#8a5d00',green='#1d7a35',sel=0.14,fill=0.04,border=0.50),
}
for n,t in themes.items():
    bg=h2rgb(t['background']); sf=h2rgb(t['lighter_background']); fg=h2rgb(t['foreground'])
    print(n)
    for k in ['foreground','muted','accent','red','yellow','green']:
        c=h2rgb(t[k]); print(f"  {k:10} on bg {cr(c,bg):5.2f}  on surface {cr(c,sf):5.2f}")
    selbg=blend(fg,sf,t['sel']); print(f"  fg on selected fill {cr(fg,selbg):5.2f}; muted on selected {cr(h2rgb(t['muted']),selbg):5.2f}")
    brd=blend(fg,bg,t['border']); print(f"  control border vs bg {cr(brd,bg):5.2f}")
    unlit=blend(fg,bg,0.12); print(f"  lit dot vs unlit dot(0.12) {cr(fg,unlit):5.2f}; unlit vs bg {cr(unlit,bg):4.2f}")
    dim=blend(fg,sf,0.5); print(f"  disabled text (fg@0.5 on surface) {cr(dim,sf):4.2f}")
