# Konu 3: loudness.rs — K-ağırlıklama, kapılama, gerçek tepe
import numpy as np, math
PI=math.pi
def k_weighting(fs):
    f0,g,q=1681.974450955533,3.999843853973347,0.7071752369554196
    k=math.tan(PI*f0/fs); vh=10**(g/20); vb=vh**0.4996667741545416
    a0=1+k/q+k*k
    shelf=([(vh+vb*k/q+k*k)/a0,2*(k*k-vh)/a0,(vh-vb*k/q+k*k)/a0],[2*(k*k-1)/a0,(1-k/q+k*k)/a0])
    f0,q=38.13547087602444,0.5003270373238773
    k=math.tan(PI*f0/fs); a0=1+k/q+k*k
    hp=([1.0,-2.0,1.0],[2*(k*k-1)/a0,(1-k/q+k*k)/a0])
    return shelf,hp
std_shelf_b=[1.53512485958697,-2.69169618940638,1.19839281085285]; std_shelf_a=[-1.69065929318241,0.73248077421585]
std_hp_b=[1.0,-2.0,1.0]; std_hp_a=[-1.99004745483398,0.99007225036621]
(sb,sa),(hb,ha)=k_weighting(48000)
print("== 48 kHz katsayıları vs BS.1770-4 Tablo 1/2 (mutlak fark) ==")
print("  raf b:",[f"{x-y:.1e}" for x,y in zip(sb,std_shelf_b)],"a:",[f"{x-y:.1e}" for x,y in zip(sa,std_shelf_a)])
print("  RLB b:",[f"{x-y:.1e}" for x,y in zip(hb,std_hp_b)],"a:",[f"{x-y:.1e}" for x,y in zip(ha,std_hp_a)])
def H(b,a,f,fs):
    z=np.exp(-1j*2*PI*f/fs); return (b[0]+b[1]*z+b[2]*z*z)/(1+a[0]*z+a[1]*z*z)
def kdb(fs,f):
    (sb,sa),(hb,ha)=k_weighting(fs); return 20*math.log10(abs(H(sb,sa,f,fs)*H(hb,ha,f,fs)))
print("== K-ağırlıklı yanıt (dB): 48k (standart) vs 44.1k / 96k / 192k ==")
print("  f(Hz)   48k      44.1k-48k  96k-48k  192k-48k")
for f in [20,38,100,500,1000,1681,5000,10000,15000,20000]:
    r48=kdb(48000,f); print(f"  {f:6d} {r48:+8.3f}  {kdb(44100,f)-r48:+8.4f} {kdb(96000,f)-r48:+8.4f} {kdb(192000,f)-r48:+8.4f}")
# RLB düz bölge kazancı (pay normalize edilmemiş): 1 kHz'te raf yanıtı 0 dB varsayımıyla
print(f"  RLB tek başına 1 kHz'te: {20*math.log10(abs(H(hb,ha,1000,48000))):+.4f} dB (standartta da pay [1,-2,1] normalize değildir)")
# Tam ölçer simülasyonu
def biquad_run(b,a,x):
    y=np.zeros_like(x); x1=x2=y1=y2=0.0
    for i,v in enumerate(x):
        o=b[0]*v+b[1]*x1+b[2]*x2-a[0]*y1-a[1]*y2; x2,x1=x1,v; y2,y1=y1,o; y[i]=o
    return y
def measure(parts,fs=48000,channels=2,freq=1000.0):
    n=0; xs=[]
    for sec,dbfs in parts:
        amp=10**(dbfs/20); N=int(sec*fs); t=(np.arange(N)+n)/fs; xs.append(amp*np.sin(2*PI*freq*t)); n+=N
    x=np.concatenate(xs)
    (sb,sa),(hb,ha)=k_weighting(fs)
    y=biquad_run(hb,ha,biquad_run(sb,sa,x))
    w=2.0 if channels==1 else 1.0
    e=w*channels*y*y if channels>1 else w*y*y
    hop=int(round(fs*0.1)); nsub=len(e)//hop
    subs=np.add.reduceat(e[:nsub*hop],np.arange(0,nsub*hop,hop))
    blocks=np.array([subs[i-3:i+1].sum()/(4*hop) for i in range(3,nsub)])
    lufs=lambda z:-0.691+10*np.log10(z)
    gated=blocks[(blocks>0)&(lufs(blocks)>-70)]
    rel=lufs(gated.mean())-10
    kept=gated[lufs(gated)>rel]
    return lufs(kept.mean())
print("== Ölçer simülasyonu (kodun adımları) ==")
print(f"  1 kHz −23 dBFS stereo 20 s → {measure([(20,-23)]):.4f} LUFS (bekl −23.0)")
print(f"  1 kHz −23 dBFS stereo @44.1k → {measure([(10,-23)],fs=44100):.4f} LUFS")
print(f"  EBU 3341 durum 3 (−36/−23/−36) → {measure([(10,-36),(60,-23),(10,-36)]):.3f} LUFS (bekl −23.0)")
print(f"  mono 1 kHz −23 dBFS (kodun ağırlığı 2) → {measure([(10,-23)],channels=1):.4f} LUFS; BS.1770 tek kanal (ağırlık 1) → {measure([(10,-23)],channels=1)-10*math.log10(2):.4f}")
# Gerçek tepe süzgeci
def truepeak_phases(fs):
    factor=4 if fs<96000 else (2 if fs<192000 else 1)
    taps=12; L=factor*taps; center=(L-1)/2
    ph=[[0.0]*taps for _ in range(factor)]
    for n in range(L):
        x=(n-center)/factor; s=1.0 if abs(x)<1e-12 else math.sin(PI*x)/(PI*x)
        w=0.42-0.5*math.cos(2*PI*n/(L-1))+0.08*math.cos(4*PI*n/(L-1))
        ph[n%factor][n//factor]=s*w
    for p in ph:
        s=sum(p); 
        for i in range(taps): p[i]/=s
    return ph
ph=truepeak_phases(48000)
print("== Gerçek tepe ara değerleyicisi (4×, 12 katsayı/faz, Blackman): faz yanıtı (dB) ==")
print("  f(kHz) "+" ".join(f"faz{p}" for p in range(4)))
for f in [1,5,10,12,15,18,20,22]:
    w=2*PI*f*1000/48000
    vals=[abs(sum(c*np.exp(-1j*w*t) for t,c in enumerate(p))) for p in ph]
    print(f"  {f:5d}  "+" ".join(f"{20*math.log10(v):+6.3f}" for v in vals))
# en kötü durumda tam ölçekli sinüsün ölçülen tepesi (faz taraması)
def meas_peak(f,fs=48000,N=48000):
    worst=99
    for ph0 in np.linspace(0,2*PI,37):
        x=np.sin(2*PI*f*np.arange(N)/fs+ph0)
        pk=np.abs(x).max()
        for p in ph:
            y=np.convolve(x,p[::-1] if False else p,mode='valid'); pk=max(pk,np.abs(y).max())
        worst=min(worst,pk)
    return 20*math.log10(worst)
print("== Tam ölçekli sinüs için ölçülen dBTP'nin en kötü (en düşük) değeri (gerçek: 0.00) ==")
for f in [997,5000,10000,12000,15000,18000,20000]:
    print(f"  {f:5d} Hz: {meas_peak(f):+.3f} dBTP")
