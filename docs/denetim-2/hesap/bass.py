# Konu 7: bass.rs oktav bölücü harmonik bozulması ve punch.rs sızıntı simülasyonu
import numpy as np, math
PI=math.pi
def norm(b,a): return [x/a[0] for x in b],[a[1]/a[0],a[2]/a[0]]
def lowpass(f,q,fs):
    w0=2*PI*f/fs; s,c=math.sin(w0),math.cos(w0); al=s/(2*q); return norm([(1-c)/2,1-c,(1-c)/2],[1+al,-2*c,1-al])
def highpass(f,q,fs):
    f=min(f,fs*0.49); w0=2*PI*f/fs; s,c=math.sin(w0),math.cos(w0); al=s/(2*q); return norm([(1+c)/2,-(1+c),(1+c)/2],[1+al,-2*c,1-al])
def lowshelf(f,g,q,fs):
    if abs(g)<1e-6: return [1,0,0],[0,0]
    A=10**(g/40); w0=2*PI*f/fs; s,c=math.sin(w0),math.cos(w0); al=s/(2*q); k=2*math.sqrt(A)*al
    return norm([A*((A+1)-(A-1)*c+k),2*A*((A-1)-(A+1)*c),A*((A+1)-(A-1)*c-k)],[(A+1)+(A-1)*c+k,-2*((A-1)+(A+1)*c),(A+1)+(A-1)*c-k])
class BQ:
    def __init__(s,ba): s.b,s.a=ba; s.z=[0.0,0.0]
    def p(s,x):
        y=s.b[0]*x+s.z[0]; s.z[0]=s.b[1]*x-s.a[0]*y+s.z[1]; s.z[1]=s.b[2]*x-s.a[1]*y; return y
fs=48000; q=1/math.sqrt(2)
class Octaver:
    def __init__(s):
        s.bl=BQ(highpass(40,q,fs)); s.bh1=BQ(lowpass(120,q,fs)); s.bh2=BQ(lowpass(120,q,fs))
        s.env=0; s.full=0; s.rel=math.exp(-1/(0.15*fs)); s.armed=False; s.sign=1.0
        s.sm1=BQ(lowpass(60,q,fs)); s.sm2=BQ(lowpass(60,q,fs)); s.dc=BQ(highpass(18,q,fs))
    def p(s,m):
        b=s.bh2.p(s.bh1.p(s.bl.p(m))); s.env=max(abs(b),s.env*s.rel); s.full=max(abs(m),s.full*s.rel)
        th=0.25*s.env
        if b<-th: s.armed=True
        elif s.armed and b>th: s.armed=False; s.sign=-s.sign
        pres=min(1.0,s.env/max(s.full*0.01,1e-12)); sq=s.sign*s.env*pres*0.8
        return s.dc.p(s.sm2.p(s.sm1.p(sq)))
def spectrum_db(y,fs):
    Y=np.abs(np.fft.rfft(y*np.hanning(len(y))))/len(y)*4; f=np.fft.rfftfreq(len(y),1/fs); return f,Y
print("== Oktav bölücü: sabit sinüs girişinde alt oktav seviyesi ve harmonikleri (giriş genliği 0.5 = −6 dBFS) ==")
for f0 in [40,55,80,100,120]:
    o=Octaver(); N=fs*4; t=np.arange(N)/fs; x=0.5*np.sin(2*PI*f0*t)
    y=np.array([o.p(v) for v in x])[fs*2:]
    f,Y=spectrum_db(y,fs)
    def lvl(fr): 
        i=np.argmin(np.abs(f-fr)); return 20*math.log10(Y[max(i-2,0):i+3].max()+1e-12)
    sub=lvl(f0/2); comps={fr:lvl(fr) for fr in [f0/2*3,f0/2*5,f0,f0/2*7]}
    print(f"  giriş {f0:3d} Hz: alt oktav {f0/2:5.1f} Hz {sub:+.1f} dBFS (girişe göre {sub+6.02:+.1f} dB); 3. harm {comps[f0/2*3]-sub:+.1f}, 5. harm {comps[f0/2*5]-sub:+.1f} dB (alt oktava göre); tepe/zarf {np.abs(y).max()/0.5:.2f}")
print("== İki tonlu bas (55 + 82.4 Hz, A1+E2) → alt oktav çıktısı spektrumu (en güçlü 5 bileşen) ==")
o=Octaver(); N=fs*4; t=np.arange(N)/fs; x=0.35*np.sin(2*PI*55*t)+0.35*np.sin(2*PI*82.4*t)
y=np.array([o.p(v) for v in x])[fs*2:]; f,Y=spectrum_db(y,fs)
idx=np.argsort(Y)[::-1]; shown=[]
for i in idx:
    if all(abs(f[i]-s)>3 for s in shown): shown.append(f[i])
    if len(shown)==5: break
print("  ",", ".join(f"{fr:.1f} Hz ({20*math.log10(Y[np.argmin(np.abs(f-fr))]):+.0f} dBFS)" for fr in shown))
print("== Harmonik üretici: Chebyshev kontrolü T2,T3,T4 ==")
u=np.cos(np.linspace(0,2*PI,1000)); th=np.linspace(0,2*PI,1000)
print(f"  maks |T2-cos2θ|={np.abs(2*u*u-1-np.cos(2*th)).max():.1e}, |T3-cos3θ|={np.abs(u*(4*u*u-3)-np.cos(3*th)).max():.1e}, |T4-cos4θ|={np.abs(8*u*u*(u*u-1)+1-np.cos(4*th)).max():.1e}")
# Punch: sürekli bas (tremolo'lu ve düz) ve bas gitar notaları
class Punch:
    def __init__(s):
        s.d1=BQ(lowpass(150,q,fs)); s.d2=BQ(lowpass(150,q,fs)); s.fast=0; s.hold=int(0.02*fs); s.hl=0
        s.frel=math.exp(-1/(0.03*fs)); s.slow=0; s.sc=1-math.exp(-1/(0.03*fs)); blk=16
        s.ac=1-math.exp(-blk/(0.002*fs)); s.rc=1-math.exp(-blk/(0.04*fs)); s.g=0; s.ul=0; s.warm=int(0.15*fs)
    def p(s,m):
        low=abs(s.d2.p(s.d1.p(m)))
        if low>=s.fast: s.fast=low; s.hl=s.hold
        elif low>=s.fast*0.891: s.hl=s.hold
        elif s.hl>0: s.hl-=1
        else: s.fast*=s.frel
        if s.warm>0: s.warm-=1; s.slow=max(s.slow,s.fast)
        elif s.fast>s.slow: s.slow+=(s.fast-s.slow)*s.sc
        else: s.slow=s.fast
        if s.ul==0:
            s.ul=16
            tgt=0.0
            if s.fast>1e-4 and s.warm==0:
                rise=20*math.log10(s.fast/max(s.slow,1e-4)); tgt=min(max(1.0*(rise-1.0),0),8.0)
            c=s.ac if tgt>s.g else s.rc; s.g+=(tgt-s.g)*c
            if tgt==0 and s.g<0.02: s.g=0
        s.ul-=1; return s.g
print("== Vuruş (punch) kazanç sızıntısı: sürekli basta raf kazancı (dB, vuruş %100) ==")
t=np.arange(fs*3)/fs
cases={"düz 55 Hz sinüs":0.5*np.sin(2*PI*55*t),
 "25 Hz sinüs (yarım dalga 20 ms sınırı)":0.5*np.sin(2*PI*25*t),
 "20 Hz sinüs":0.5*np.sin(2*PI*20*t),
 "55 Hz, 6 Hz tremolo ±1.5 dB":0.5*np.sin(2*PI*55*t)*10**((1.5*np.sin(2*PI*6*t))/20),
 "55 Hz, 6 Hz tremolo ±3 dB":0.5*np.sin(2*PI*55*t)*10**((3*np.sin(2*PI*6*t))/20),
 "bas gitar: 0.5 s notalar, 5 ms atak, hafif sönüm":0.5*np.sin(2*PI*55*t)*np.minimum(1,(t%0.5)/0.005)*np.exp(-(t%0.5)*0.6),
 "55 Hz + 110 Hz vuruş (gerçek kick)":0.5*np.sin(2*PI*55*t)*(1+np.where((t%0.5)<0.08,1.0,0.0)*np.exp(-(t%0.5)/0.03))}
for name,x in cases.items():
    p=Punch(); g=np.array([p.p(v) for v in x])[fs:]  # ilk 1 s (ısınma) atılır
    print(f"  {name:48s}: maks {g.max():.2f} dB, ortalama {g.mean():.2f} dB, %>0.5 dB: {100*(g>0.5).mean():.0f}%")
