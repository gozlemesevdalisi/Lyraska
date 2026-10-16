# Konu 6: rubato 5.0.1 Fft yeniden örnekleyicinin süzgeci (synchro.rs + sinc.rs + windows.rs okunarak)
import numpy as np, math
PI=math.pi
def cutoff(n): k1,k2,k3=13.745202940783823,121.73532586374934,5964.163279612051; return 1/(k1/n+k2/n**2+k3/n**3+1)
def bh2(N):  # periyodik Blackman-Harris, karesi
    n=np.arange(N); w=0.35875-0.48829*np.cos(2*PI*n/N)+0.14128*np.cos(4*PI*n/N)-0.01168*np.cos(6*PI*n/N); return w*w
def sinc(x): return np.where(np.abs(x)<1e-12,1.0,np.sin(PI*x)/(PI*x))
def filt(nin,nout):
    c = cutoff(nin) if nin<nout else cutoff(nout)*nout/nin
    x=np.arange(nin)-nin//2; h=bh2(nin)*sinc(x*c); h/=h.sum(); return h,c
def analyze(fs_in,fs_out,chunk=2048):
    g=math.gcd(fs_in,fs_out); mci=fs_in//g; chunks=-(-chunk//mci)
    nin=chunks*fs_in//g; nout=chunks*fs_out//g
    h,c=filt(nin,nout)
    Hf=np.fft.rfft(h,1<<18); f=np.fft.rfftfreq(1<<18,1/fs_in); mag=20*np.log10(np.abs(Hf)+1e-300)
    print(f"=== {fs_in}→{fs_out}: fft_size_in={nin}, fft_size_out={nout}, bağıl kesme c={c:.5f} → {c*fs_in/2:.0f} Hz; çıkış gecikmesi {nout//2} çıkış karesi ({nout//2/fs_out*1000:.1f} ms) ===")
    def at(fr): return mag[np.argmin(np.abs(f-fr))]
    for fr in [1000,10000,15000,19000,20000,21000,21500,22000,22050]:
        if fr<fs_in/2: print(f"  {fr:6d} Hz: {at(fr):+9.3f} dB")
    pb=f[(f<=20000)]; print(f"  20 Hz–20 kHz geçirme bandı dalgalanması: {mag[(f>=20)&(f<=20000)].max():+.4f}/{mag[(f>=20)&(f<=20000)].min():+.4f} dB")
    # -0.1 dB ve -100 dB noktaları
    i01=np.where(mag<-0.1)[0][0]; i100=np.where(mag<-100)[0][0]
    print(f"  −0.1 dB noktası {f[i01]:.0f} Hz, −100 dB noktası {f[i100]:.0f} Hz → geçiş bandı {f[i100]-f[i01]:.0f} Hz")
    ny_out=fs_out/2
    if fs_out<fs_in:
        stop=mag[f>=ny_out]; print(f"  Çıkış Nyquist'inin ({ny_out:.0f} Hz) üstünde en yüksek seviye (alias): {stop.max():+.1f} dB; 23 kHz'te {at(23000):+.1f} dB")
        print(f"  Katlanma: {ny_out:.0f}–{fs_in/2:.0f} Hz bandındaki içerik {fs_out-fs_in/2:.0f}–{ny_out:.0f} Hz'e düşer; o bantta süzgeç {mag[(f>=ny_out)&(f<=fs_in/2)].max():+.1f} dB (en kötü)")
    else:
        # üst örnekleme: giriş Nyquist'i üstü görüntüler çıkış süzgecinde (aynı h) zayıflatılır
        img=mag[(f>=fs_in/2-500)&(f<=fs_in/2)]; print(f"  giriş Nyquist'ine ({fs_in/2:.0f}) 500 Hz kala süzgeç: {img.min():+.1f}..{img.max():+.1f} dB (aynalanan görüntü seviyesi)")
analyze(44100,48000); analyze(48000,44100); analyze(96000,48000); analyze(44100,96000)
