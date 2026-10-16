# Konu 2: eq.rs tasarımının Python'a bire bir taşınması; hedef eğri ile gerçekleşen arasındaki fark
import numpy as np, math
PI=math.pi
BANDS=10; CENTERS=[31.25,62.5,125,250,500,1000,2000,4000,8000,16000]
MAX_POINTS=19; MAX_CENTER_RATIO=0.4; MIN_SHAPE=0.5; FLAT=1e-3
T=dict(bandwidth_ratio=1.0,edge_ratio=0.4,limit=1.8,prototype_db=12.0,passes=2)
def lin(db): return 10**(db/20)
class BQ:
    def __init__(s,b0=1,b1=0,b2=0,a1=0,a2=0): s.b0,s.b1,s.b2,s.a1,s.a2=b0,b1,b2,a1,a2
    @staticmethod
    def peaking(w0,bw,g_db,edge):
        if abs(g_db)<FLAT: return BQ()
        g=lin(g_db); gb=lin(edge*g_db)
        beta=math.tan(bw/2)*math.sqrt(abs(gb*gb-1)/abs(g*g-gb*gb))
        n=1/(1+beta); c=-2*math.cos(w0)*n
        return BQ((1+g*beta)*n,c,(1-g*beta)*n,c,(1-beta)*n)
    def resp(s,cw,c2w):
        num=s.b0**2+s.b1**2+s.b2**2+2*(s.b0*s.b1+s.b1*s.b2)*cw+2*s.b0*s.b2*c2w
        den=1+s.a1**2+s.a2**2+2*(s.a1+s.a1*s.a2)*cw+2*s.a2*c2w
        return 10*math.log10(max(num,1e-300)/max(den,1e-300))
def dp_hz(k): return CENTERS[k//2] if k%2==0 else math.sqrt(CENTERS[k//2]*CENTERS[k//2+1])
def dtarget(g,k): return g[k//2] if k%2==0 else 0.5*(g[k//2]+g[k//2+1])
class Layout:
    def __init__(s,fs):
        s.bands=sum(1 for f in CENTERS if f<MAX_CENTER_RATIO*fs)
        s.points=max(2*s.bands-1,0)
        s.w0=[2*PI*f/fs for f in CENTERS[:s.bands]]
        s.bw=[min(T['bandwidth_ratio']*w,T['limit']*(PI-w)) for w in s.w0]
        s.cw=[math.cos(2*PI*dp_hz(k)/fs) for k in range(s.points)]
        s.c2w=[math.cos(4*PI*dp_hz(k)/fs) for k in range(s.points)]
def solve(layout,shape,target):
    n,p=layout.bands,layout.points
    A=np.zeros((p,n))
    for m in range(n):
        f=BQ.peaking(layout.w0[m],layout.bw[m],shape[m],T['edge_ratio'])
        for k in range(p): A[k,m]=f.resp(layout.cw[k],layout.c2w[k])/shape[m]
    N=A.T@A; r=A.T@np.array(target[:p])
    return np.linalg.solve(N,r)
def design(gains,fs):
    L=Layout(fs); n=L.bands
    if all(abs(g)<FLAT for g in gains[:n]): return [BQ()]*BANDS,L
    target=[dtarget(gains,k) for k in range(L.points)]
    g=solve(L,[T['prototype_db']]*BANDS,target)
    for _ in range(1,T['passes']):
        shape=[ (math.copysign(MIN_SHAPE,x) if abs(x)<MIN_SHAPE else x) for x in g]
        g=solve(L,shape,target)
    return [BQ.peaking(L.w0[m],L.bw[m],g[m],T['edge_ratio']) for m in range(n)],L
def response(filters,hz,fs):
    w=2*PI*hz/fs; return sum(f.resp(math.cos(w),math.cos(2*w)) for f in filters)
def target_curve(gains,hz):
    # hedef: bant ortalarında sürgü, arada log-frekansta doğrusal ara değer (kodun tasarım hedefinin doğal uzantısı)
    x=math.log2(hz); xs=[math.log2(c) for c in CENTERS]
    if x<=xs[0]: return gains[0]
    if x>=xs[-1]: return gains[-1]
    i=max(j for j in range(9) if xs[j]<=x); t=(x-xs[i])/(xs[i+1]-xs[i]); return gains[i]+(gains[i+1]-gains[i])*t
settings={
 "hepsi +6":[6]*10, "hepsi +12":[12]*10, "hepsi -12":[-12]*10,
 "zikzak ±12":[12,-12]*5, "bas +6 (6,4,2,0..)":[6,4,2,0,0,0,0,0,0,0],
 "gülümseme (6,4,2,0,-2,-2,0,2,4,6)":[6,4,2,0,-2,-2,0,2,4,6],
 "tek bant 1 kHz +12":[0,0,0,0,0,12,0,0,0,0], "tek bant 31 Hz +12":[12,0,0,0,0,0,0,0,0,0],
 "tek bant 16 kHz +12":[0]*9+[12], "yarım-yarım (+12/-12)":[12]*5+[-12]*5,
 "yumuşak (3,3,2,1,0,0,1,2,3,3)":[3,3,2,1,0,0,1,2,3,3],
}
for fs in (44100,48000,96000):
    print(f"=== fs={fs} ===")
    print(f"  {'ayar':40s} tasarım noktası maks | 1/24 okt ızgara (20 Hz–0.45fs) maks | tüm sürgüler eşitken orta değer")
    for name,g in settings.items():
        F,L=design(g,fs)
        e_dp=max(abs(response(F,dp_hz(k),fs)-dtarget(g,k)) for k in range(L.points))
        grid=np.geomspace(20,min(20000,0.45*fs),400)
        e_grid=max(abs(response(F,h,fs)-target_curve(g,h)) for h in grid)
        mid=np.mean([response(F,h,fs) for h in np.geomspace(40,8000,100)])
        print(f"  {name:40s} {e_dp:6.3f} dB | {e_grid:6.3f} dB | {mid:+.2f} dB")
# "hepsi +6" sıradan kaskad (etkileşimsiz) ile karşılaştırma: ne kadar şişerdi?
fs=48000; L=Layout(fs)
F=[BQ.peaking(L.w0[m],L.bw[m],6.0,T['edge_ratio']) for m in range(10)]
print(f"Etkileşim düzeltmesiz kaskad, hepsi +6: 1 kHz'te {response(F,1000,fs):+.2f} dB (düzeltmeli ~+6.0)")
# Nyquist'e yakın bantta kazanç: 16 kHz +12 @44.1k; 20 kHz'teki değer
F,L=design([0]*9+[12],44100); print(f"16 kHz +12 @44.1k: 16 kHz {response(F,16000,44100):+.2f}, 20 kHz {response(F,20000,44100):+.2f}, 11.3 kHz (ara nokta) {response(F,math.sqrt(8000*16000),44100):+.2f} (hedef +6)")
# tüm +12: 20 Hz ve 20 kHz'teki değer (uçlar)
F,L=design([12]*10,48000); print(f"hepsi +12 @48k: 20 Hz {response(F,20,48000):+.2f}, 20 kHz {response(F,20000,48000):+.2f}, 1 kHz {response(F,1000,48000):+.2f}")
