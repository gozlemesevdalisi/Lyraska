import numpy as np, math
exec(open('rbj.py').read().split('fs=48000')[0])
def peak(filters,fs):
    return max(sum(db(H(*f(fs),h,fs)) for f in filters) for h in np.geomspace(20,20000,2000) if h<fs*0.49)
# AutoEq tipik yüksek frekans filtreleri
prof=[lambda fs:highshelf(10000,6.0,0.7,fs), lambda fs:peaking(16000,6.0,1.41,fs), lambda fs:peaking(8000,5.0,1.0,fs), lambda fs:peaking(19000,8.0,3.0,fs)]
for name,fl in [("HSC 10k +6 Q0.7",prof[:1]),("PK 16k +6 Q1.41",prof[1:2]),("PK 19k +8 Q3",prof[3:]),("hepsi birlikte",prof)]:
    p96=peak(fl,96000); p44=peak(fl,44100); p48=peak(fl,48000)
    print(f"  {name:18s}: tepe 96k {p96:+.2f} dB (ön kazanç buna göre), 48k {p48:+.2f}, 44.1k {p44:+.2f} → 44.1k'da fark {p44-p96:+.2f} dB")
