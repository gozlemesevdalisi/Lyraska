# Konu 5: limiter.rs birebir simülasyon
import numpy as np, math
PI=math.pi
class Limiter:
    def __init__(s,ch,fs):
        s.ch=ch; s.delay=max(1,round(0.0015*fs)); s.release=1-math.exp(-1/(0.08*fs))
        s.line=np.zeros(s.delay*ch); s.lp=0; s.wanted=np.ones(s.delay+1); s.wp=0
        s.sm=np.ones(s.delay); s.sp=0; s.ssum=float(s.delay); s.held=1.0; s.gain=1.0
    def frame(s,fr):
        peak=np.abs(fr).max(); wanted=1.0/peak if peak>1.0 else 1.0
        s.wanted[s.wp]=wanted; s.wp=(s.wp+1)%len(s.wanted); mn=min(1.0,s.wanted.min())
        s.held=mn if mn<s.held else s.held+(mn-s.held)*s.release
        s.ssum+=s.held-s.sm[s.sp]; s.sm[s.sp]=s.held; s.sp=(s.sp+1)%s.delay
        g=s.ssum/s.delay
        if g>1-1e-12: g=1.0
        s.gain=g; base=s.lp*s.ch; out=fr.copy()
        for c in range(s.ch):
            d=s.line[base+c]; s.line[base+c]=fr[c]; out[c]=d if g==1.0 else d*g
        s.lp=(s.lp+1)%s.delay; return out
def run(x,ch,fs):
    L=Limiter(ch,fs); out=np.zeros_like(x)
    for i in range(0,len(x),ch): out[i:i+ch]=L.frame(x[i:i+ch])
    return out
fs=48000
rng=np.random.default_rng(1)
# 1) Düşmanca: rastgele darbeler + +6 dB kare dalga + 1.9 tepeler
N=fs
x=1.4*np.sin(2*PI*100*np.arange(N)/fs)*np.abs(np.sin(2*PI*0.7*np.arange(N)/fs))
x[::5000]=1.9; x[20000:21000]=np.where(((np.arange(1000)//50)%2)==0,1.3,-1.3)
x+= rng.normal(0,0.3,N)*(rng.random(N)<0.01)*5
out=run(x,1,fs); print(f"düşmanca sinyal: giriş maks {np.abs(x).max():.2f}, çıkış maks {np.abs(out).max():.9f} (≤1.0 → örnek tepe ispatı tutuyor)")
# 2) Kazanç sürekliliği: en büyük adım
L=Limiter(1,fs); g=[]; xx=np.full(fs,0.5); xx[1000]=2.0
for v in xx: L.frame(np.array([v])); g.append(L.gain)
g=np.array(g); print(f"tek darbe 2.0: min kazanç {g.min():.4f}, en büyük kazanç adımı {np.abs(np.diff(g)).max():.5f} (= 0.5/{L.delay} = {0.5/L.delay:.5f})")
# 3) Örnekler arası tepe: fs/4 sinüs 45° kaydırmalı, genlik 1.4 → örnek tepe 0.99
amp=0.99/math.sin(PI/4); x=amp*np.sin(PI/2*np.arange(fs)+PI/4)
out=run(x,1,fs)
# 4x ara değerleme ile gerçek tepe (ideal sinc, uzun)
def true_peak(y,factor=16):
    Y=np.fft.rfft(y); n=len(y); Yu=np.zeros(n*factor//2+1,dtype=complex); Yu[:len(Y)]=Y*factor
    return np.abs(np.fft.irfft(Yu,n*factor)).max()
print(f"fs/4 sinüs, örnek tepe {np.abs(x).max():.3f} (−0.09 dBFS), gerçek tepe {20*math.log10(true_peak(x)):+.2f} dBTP → sınırlayıcı kazancı {run(x,1,fs)[-1]/x[-1-L.delay]:.3f}; çıkış gerçek tepe {20*math.log10(true_peak(out[2000:])):+.2f} dBTP")
# 4) Gerçekçi: 0 dBFS'e yakın basılmış sinyal → sınırlayıcı sonrasında gerçek tepe
x=np.clip(1.3*np.sin(2*PI*9000*np.arange(fs)/fs)*(0.6+0.4*np.sin(2*PI*3*np.arange(fs)/fs)),-1,1)  # sert kırpılmış 9 kHz
out=run(x,1,fs); print(f"kırpılmış 9 kHz: giriş örnek tepe {np.abs(x).max():.3f}, gerçek tepe {20*math.log10(true_peak(x)):+.2f} dBTP; çıkış örnek tepe {np.abs(out).max():.3f}, gerçek tepe {20*math.log10(true_peak(out[2000:])):+.2f} dBTP")
print(f"gecikme {L.delay} kare = {L.delay/fs*1000:.3f} ms; bırakma sabiti 80 ms")
