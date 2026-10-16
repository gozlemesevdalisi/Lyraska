# Konu 1: biquad.rs'nin RBJ formülleri — bağımsız doğrulama
import numpy as np, math
PI=math.pi
def norm(b,a): return [x/a[0] for x in b],[a[1]/a[0],a[2]/a[0]]
def shape(f,g,q,fs):
    A=10**(g/40); w0=2*PI*f/fs; s,c=math.sin(w0),math.cos(w0); return A,c,s/(2*q),s
def peaking(f,g,q,fs):
    A,c,al,_=shape(f,g,q,fs); return norm([1+al*A,-2*c,1-al*A],[1+al/A,-2*c,1-al/A])
def lowshelf(f,g,q,fs):
    A,c,al,_=shape(f,g,q,fs); k=2*math.sqrt(A)*al
    return norm([A*((A+1)-(A-1)*c+k),2*A*((A-1)-(A+1)*c),A*((A+1)-(A-1)*c-k)],
                [(A+1)+(A-1)*c+k,-2*((A-1)+(A+1)*c),(A+1)+(A-1)*c-k])
def highshelf(f,g,q,fs):
    A,c,al,_=shape(f,g,q,fs); k=2*math.sqrt(A)*al
    return norm([A*((A+1)+(A-1)*c+k),-2*A*((A-1)+(A+1)*c),A*((A+1)+(A-1)*c-k)],
                [(A+1)-(A-1)*c+k,2*((A-1)-(A+1)*c),(A+1)-(A-1)*c-k])
def lowpass(f,q,fs):
    w0=2*PI*f/fs; s,c=math.sin(w0),math.cos(w0); al=s/(2*q)
    return norm([(1-c)/2,1-c,(1-c)/2],[1+al,-2*c,1-al])
def highpass(f,q,fs):
    w0=2*PI*f/fs; s,c=math.sin(w0),math.cos(w0); al=s/(2*q)
    return norm([(1+c)/2,-(1+c),(1+c)/2],[1+al,-2*c,1-al])
def H(b,a,f,fs):
    z=np.exp(-1j*2*PI*f/fs)
    return (b[0]+b[1]*z+b[2]*z*z)/(1+a[0]*z+a[1]*z*z)
def db(x): return 20*math.log10(abs(x))
# Kodun response_db formülü (cos tabanlı) ile doğrudan z-dönüşümü karşılaştırması
def response_db_code(b,a,w):
    b0,b1,b2=b; a1,a2=a; cw,c2w=math.cos(w),math.cos(2*w)
    num=b0*b0+b1*b1+b2*b2+2*(b0*b1+b1*b2)*cw+2*b0*b2*c2w
    den=1+a1*a1+a2*a2+2*(a1+a1*a2)*cw+2*a2*c2w
    return 10*math.log10(num/den)
fs=48000
print("== response_db formülü vs |H(z)| (en büyük fark, dB) ==")
worst=0
for (b,a) in [peaking(1000,6,1,fs),lowshelf(100,9,0.707,fs),highshelf(8000,-6,0.707,fs),lowpass(120,0.7071,fs),highpass(120,0.7071,fs)]:
    for f in np.geomspace(10,23000,300):
        worst=max(worst,abs(response_db_code(b,a,2*PI*f/fs)-db(H(b,a,f,fs))))
print(f"  {worst:.2e} dB")
print("== Peaking: fc'de kazanç ve bant genişliği (RBJ: BW kenarları G/2 dB'de, 1/Q = 2 sinh(ln2/2·BW·w0/sin w0)) ==")
for fc,g,q in [(1000,6,1.0),(105,-2.3,0.7),(3500,12,2.0),(16000,6,1.41)]:
    b,a=peaking(fc,g,q,fs); gc=db(H(b,a,fc,fs))
    # RBJ Q<->BW: 1/Q = 2 sinh( ln2/2 * BW * w0/sin(w0) )
    w0=2*PI*fc/fs; BW=2/(math.log(2)*w0/math.sin(w0))*math.asinh(1/(2*q))
    fl,fh=fc*2**(-BW/2),fc*2**(BW/2)
    print(f"  fc={fc} G={g} Q={q}: H(fc)={gc:+.4f} dB; BW={BW:.3f} okt → {fl:.0f}/{fh:.0f} Hz'de {db(H(b,a,fl,fs)):+.3f}/{db(H(b,a,fh,fs)) if fh<fs/2 else float('nan'):+.3f} dB (beklenen {g/2:+.2f})")
print("== Raflar: fc'de G/2, uzakta G / 0 ==")
for fn,fc,g in [(lowshelf,100,9),(lowshelf,105,5.5),(highshelf,8000,-6),(highshelf,10000,2)]:
    b,a=fn(fc,g,0.7071,fs)
    far_lo,far_hi=db(H(b,a,5,fs)),db(H(b,a,23900,fs))
    print(f"  {fn.__name__} fc={fc} G={g}: H(fc)={db(H(b,a,fc,fs)):+.3f} (bekl {g/2:+.2f}); 5 Hz {far_lo:+.3f}; 23,9 kHz {far_hi:+.3f}")
print("== LP/HP Butterworth: fc'de -3.01 dB, 10 fc'de -40 dB ==")
b,a=lowpass(120,1/math.sqrt(2),fs); print(f"  LP 120: {db(H(b,a,120,fs)):+.3f} dB @fc, {db(H(b,a,1200,fs)):+.2f} dB @10fc")
b,a=highpass(120,1/math.sqrt(2),fs); print(f"  HP 120: {db(H(b,a,120,fs)):+.3f} dB @fc, {db(H(b,a,12,fs)):+.2f} dB @fc/10")
print("== Kararlılık: kutup yarıçapı (uç parametreler) ==")
for fs_ in [44100,192000]:
  for fc,g,q,fn in [(10,30,30,peaking),(10,-30,30,peaking),(20,30,0.05,peaking),(10,30,0.05,lowshelf),(24000*0.49,30,30,peaking),(20,0,0.05,lowpass),(20,0,30,highpass)]:
    b,a = fn(fc,g,q,fs_) if fn in (peaking,lowshelf,highshelf) else fn(fc,q,fs_)
    r=np.abs(np.roots([1,a[0],a[1]])).max()
    print(f"  fs={fs_} {fn.__name__} fc={fc:.0f} G={g} Q={q}: |kutup|max={r:.12f} {'KARARLI' if r<1 else 'KARARSIZ'}  1-a2={1-a[1]:.3e}")
print("== f64 hassasiyeti: 20 Hz, Q=0.707, +6 dB, 192 kHz: katsayı yuvarlama hatasının yanıta etkisi ==")
b,a=peaking(20,6,0.707,192000)
eps=2.2e-16
# a1,a2'yi eps kadar oynatarak fc'deki kazanç değişimi
for d in [eps,1e-9,1e-7]:
    b2=b; a2=[a[0]*(1+d),a[1]*(1-d)]
    print(f"  katsayı bağıl hata {d:.0e}: H(fc) sapması {db(H(b2,a2,20,192000))-db(H(b,a,20,192000)):+.2e} dB (f32 eps≈1.2e-7)")
print(f"  a1={a[0]:.15f} a2={a[1]:.15f}  (1-a2 = {1-a[1]:.3e}; f64 ile 1-a2'nin bağıl hatası ≈ {eps/(1-a[1]):.1e})")
