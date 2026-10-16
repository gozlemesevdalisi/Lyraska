# Konu 10: Camelot çarkı; Konu 13: binom p-değeri (abx.rs::binomial_tail)
import math
NAMES=["C","C#","D","D#","E","F","F#","G","G#","A","A#","B"]
def camelot_number(tonic, minor):
    major_tonic = ((tonic+3) if minor else tonic) % 12
    return ((7*major_tonic) % 12 + 7) % 12 + 1
# Referans Camelot çarkı (Mixed In Key): 1A Ab min,1B B maj; 2A Eb min,2B F# maj; 3A Bb min,3B Db maj; 4A F min,4B Ab maj;
# 5A C min,5B Eb maj; 6A G min,6B Bb maj; 7A D min,7B F maj; 8A A min,8B C maj; 9A E min,9B G maj; 10A B min,10B D maj;
# 11A F# min,11B A maj; 12A Db min,12B E maj
ref={ (8,"B"):"C",(9,"B"):"G",(10,"B"):"D",(11,"B"):"A",(12,"B"):"E",(1,"B"):"B",(2,"B"):"F#",(3,"B"):"C#",(4,"B"):"G#",(5,"B"):"D#",(6,"B"):"A#",(7,"B"):"F",
      (8,"A"):"A",(9,"A"):"E",(10,"A"):"B",(11,"A"):"F#",(12,"A"):"C#",(1,"A"):"G#",(2,"A"):"D#",(3,"A"):"A#",(4,"A"):"F",(5,"A"):"C",(6,"A"):"G",(7,"A"):"D"}
ok=True
for minor in (False,True):
    for t in range(12):
        n=camelot_number(t,minor); letter="A" if minor else "B"
        good = ref[(n,letter)]==NAMES[t]
        ok&=good
        print(f"  {NAMES[t]:>2} {'min' if minor else 'maj'} → {n}{letter}  {'✓' if good else '✗ beklenen '+ref[(n,letter)]}")
print("Camelot 24/24 doğru" if ok else "Camelot HATALI")
def binomial_tail(correct,total):
    if correct==0: return 1.0
    if correct>total: return 0.0
    n=total; term=0.5**total; tail=0.0
    for k in range(total+1):
        if k>=correct: tail+=term
        term*= (n-k)/(k+1)
    return min(tail,1.0)
def exact(c,n): return sum(math.comb(n,k) for k in range(c,n+1))/2**n
print("== binom kuyruğu: kod vs math.comb ==")
worst=0
for n,c in [(16,12),(16,16),(16,8),(10,9),(50,33),(50,50),(4,4),(16,0)]:
    a,b=binomial_tail(c,n),exact(c,n); worst=max(worst,abs(a-b))
    print(f"  {c}/{n}: kod={a:.6f} tam={b:.6f}")
print(f"  en büyük mutlak fark {worst:.1e}")
