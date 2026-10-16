import numpy as np, math
exec(open('rbj.py').read().split('fs=48000')[0])
fs=48000
for name,(b,a) in [("peaking",peaking(1000,6,1,fs)),("lowshelf",lowshelf(100,9,0.707,fs)),("highshelf",highshelf(8000,-6,0.707,fs)),("lowpass",lowpass(120,0.7071,fs)),("highpass",highpass(120,0.7071,fs))]:
    worst=(0,0,0)
    for f in np.geomspace(10,23000,300):
        d=abs(response_db_code(b,a,2*PI*f/fs)-db(H(b,a,f,fs)))
        if d>worst[0]: worst=(d,f,db(H(b,a,f,fs)))
    print(f"  {name}: en büyük fark {worst[0]:.2e} dB @ {worst[1]:.0f} Hz (|H|={worst[2]:+.1f} dB)")
