# Konu 8: Ellis önseli; F-ölçüsü açgözlü eşleme karşı örneği. Konu 11: yönetmen nabız hızı.
import math, numpy as np
fps=60.0
def prior(bpm,sigma): lag=fps*60/bpm; pref=fps*60/120; return math.exp(-0.5*(math.log2(lag/pref)/sigma)**2)
print("== Tempo önseli W(τ)=exp(-½(log2(τ/0.5s)/σ)²): kod σ=1.0 okt, Ellis 2007 σ=1.4 okt ==")
for bpm in [60,80,100,120,150,180,200]:
    print(f"  {bpm:3d} BPM: kod {prior(bpm,1.0):.3f}  Ellis {prior(bpm,1.4):.3f}  oran {prior(bpm,1.0)/prior(bpm,1.4):.2f}")
print("  → Kod uçlardaki tempoları Ellis'e göre ~2× daha fazla cezalandırır (60/200 BPM'de 0.79 vs 0.60... bkz. oran).")
print("== 2τ terimine uygulanan ağırlık: kod W(τ)·0.5·ACF(2τ); Ellis 0.5·W(2τ)·ACF(2τ) ==")
for bpm in [160,120,80]:
    print(f"  τ={bpm} BPM: W(τ)={prior(bpm,1.0):.3f} vs W(2τ)=W({bpm/2:.0f} BPM)={prior(bpm/2,1.0):.3f}")
# F-ölçüsü: kodun açgözlü eşleştirmesi
def f_measure_code(est,truth,tol):
    if not est or not truth: return 0.0
    used=[False]*len(est); start=0; hits=0
    for t in truth:
        while start<len(est) and est[start]<t-tol: start+=1
        cands=[i for i in range(start,len(est)) if est[i]<=t+tol and not used[i]]
        if cands:
            i=min(cands,key=lambda i:abs(est[i]-t)); used[i]=True; hits+=1
    if hits==0: return 0.0
    p=hits/len(est); r=hits/len(truth); return 2*p*r/(p+r)
def f_measure_optimal(est,truth,tol):
    # sıralı listelerde en büyük eşleme: ardışık açgözlü (en erken eşle) optimumdur
    i=j=0; hits=0
    while i<len(est) and j<len(truth):
        if est[i]<truth[j]-tol: i+=1
        elif est[i]>truth[j]+tol: j+=1
        else: hits+=1; i+=1; j+=1
    if hits==0: return 0.0
    p=hits/len(est); r=hits/len(truth); return 2*p*r/(p+r)
print("== F-ölçüsü: 'en yakın' açgözlü (kod) vs en büyük eşleme (mir_eval) ==")
for est,truth in [([0.95,1.04],[1.00,1.05]),([0.96,1.03,1.10],[1.00,1.06,1.12]),(list(np.arange(0,30,0.5)),list(np.arange(0.05,30,0.5)))]:
    print(f"  est={est[:3]}… truth={truth[:3]}…: kod F={f_measure_code(est,truth,0.07):.3f}, optimum F={f_measure_optimal(est,truth,0.07):.3f}")
# Yönetmen: nabız + vurgu birleşik olay hızı (vuruşlar seyreltilir, ölçü başları ayrı seyreltilir)
def thin(ev,gap):
    out=[]
    for e in ev:
        if not out or e-out[-1]>=gap: out.append(e)
    return out
print("== Yönetmen: nabız(0.34 s seyreltme) + vurgu(ölçü başı, ayrı seyreltme) birleşik ayrı olay sayısı / s (herhangi 1 s pencerede en kötü) ==")
worst=(0,0,0)
for meter in (3,4,5,7):
  for bpm in range(60,241,2):
    beat=60/bpm; beats=[i*beat for i in range(int(60/beat))]
    pulses=thin(beats,0.34); accents=thin(beats[::meter],0.34)
    events=sorted(set(round(e,9) for e in pulses+accents))
    ev=np.array(events); mx=max(((ev>=t)&(ev<t+1.0)).sum() for t in ev)
    if mx>worst[0]: worst=(mx,bpm,meter)
    if bpm in (120,170,180,200,240) and meter in (3,4): print(f"  {bpm} BPM {meter}/4: nabız {len(pulses)}/dk, vurgu {len(accents)}/dk → 1 s'de en çok {mx} ayrı olay")
print(f"  En kötü (60–240 BPM, ölçü 3/4/5/7): {worst[0]} olay/s @ {worst[1]} BPM, ölçü {worst[2]}")
print("== Yönetmen: drop anında warmth sıçraması: 0.15·release (release 0→1) = 0.15; mood yarı ölçüde smoothstep (sürekli) ==")
