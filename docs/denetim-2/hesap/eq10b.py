import numpy as np, math
src=open('eq10.py').read().split('settings={')[0]; exec(src)
settings={"hepsi +6":[6]*10,"hepsi +12":[12]*10,"zikzak ±12":[12,-12]*5,"bas +6 (6,4,2,0..)":[6,4,2,0,0,0,0,0,0,0],
 "gülümseme":[6,4,2,0,-2,-2,0,2,4,6],"tek bant 1 kHz +12":[0,0,0,0,0,12,0,0,0,0],"tek bant 31 Hz +12":[12,0,0,0,0,0,0,0,0,0],
 "yarım-yarım":[12]*5+[-12]*5,"yumuşak":[3,3,2,1,0,0,1,2,3,3],"komşu zıt (0,0,0,0,+12,-12,0..)":[0,0,0,0,12,-12,0,0,0,0]}
for fs in (44100,48000):
    print(f"=== fs={fs}: 31.25 Hz–16 kHz arası 1/24 oktav ızgarada hedef(log-doğrusal)−gerçek ===")
    for name,g in settings.items():
        F,L=design(g,fs); grid=np.geomspace(31.25,16000,500)
        errs=[(response(F,h,fs)-target_curve(g,h),h) for h in grid]
        e,h=max(errs,key=lambda x:abs(x[0])); e_dp=max(abs(response(F,dp_hz(k),fs)-dtarget(g,k)) for k in range(L.points))
        print(f"  {name:32s} tasarım noktası maks {e_dp:5.2f} dB | ızgara maks {e:+5.2f} dB @ {h:7.0f} Hz")
# Tasarım noktası sapmasını tüm 300 rastgele ayar için (kodun testi gibi) en kötü değer
rng=np.random.default_rng(42)
for fs in (44100,48000,96000):
    worst=0
    for _ in range(300):
        g=list(rng.uniform(-12,12,10)); F,L=design(g,fs)
        worst=max(worst,max(abs(response(F,dp_hz(k),fs)-dtarget(g,k)) for k in range(L.points)))
    print(f"fs={fs}: 300 rastgele ayarda tasarım noktası en kötü sapma {worst:.2f} dB")
# 20 Hz ve 20 kHz'te: sürgü +12 iken ne kadar gerçekleşiyor
F,L=design([12]*10,48000); print(f"hepsi +12: 20 Hz'te {response(F,20,48000):+.2f} dB, 25 Hz {response(F,25,48000):+.2f}, 20 kHz {response(F,20000,48000):+.2f} dB")
