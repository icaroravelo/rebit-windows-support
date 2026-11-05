#!/usr/bin/env python3
"""Analyze coherent samples from the quality_spectrum Rust example.
Requires numpy/matplotlib as analysis tools, not plugin dependencies.
"""
import csv
import json
import sys
from pathlib import Path
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
root = Path(sys.argv[1] if len(sys.argv)>1 else 'dist/quality-analysis')
rows = list(csv.DictReader((root/'index.csv').open()))
results = []
fig, axes = plt.subplots(2, 2, figsize=(13, 8), constrained_layout=True)
for row in rows:
    n, rate, fundamental = int(row['frames']), float(row['rate']), int(row['bin'])
    x = np.fromfile(root/row['file'], dtype='<f4').astype(float)
    assert len(x)==n and np.all(np.isfinite(x))
    spectrum = np.fft.rfft(x)
    power = abs(spectrum)**2
    power[1:-1] *= 2
    harmonics = np.arange(fundamental, n//2, fundamental)
    harmonic_power = power[harmonics].sum()
    alias_mask = np.ones(len(power), dtype=bool)
    alias_mask[0] = False
    alias_mask[harmonics] = False
    alias_power = power[alias_mask].sum()
    db = 10*np.log10(max(alias_power, 1e-30)/harmonic_power)
    results.append(dict(shape=row['shape'],mode=row['mode'],frequency_hz=fundamental*rate/n,alias_to_harmonic_db=float(db)))
    if fundamental==2047:
        ax = axes.flat[['pulse125','pulse25','pulse50','triangle'].index(row['shape'])]
        amplitude = abs(spectrum)*2/n
        ax.plot(np.fft.rfftfreq(n,1/rate)[1:],20*np.log10(np.maximum(amplitude[1:],1e-8)),
                color='#b86732' if row['mode']=='raw' else '#087f9b',lw=.7,alpha=.8,label=row['mode'].title())
        title={"pulse125":"Pulse 12.5%","pulse25":"Pulse 25%","pulse50":"Pulse 50%","triangle":"Triangle"}[row["shape"]]
        ax.set(title=f"{title} — {fundamental*rate/n:.1f} Hz",xlabel='Frequency (Hz)',ylabel='Amplitude (dBFS)',ylim=(-100,6),xlim=(0,rate/2))
        ax.grid(alpha=.2);ax.legend()
(root/'metrics.json').write_text(json.dumps(results,indent=2)+'\n')
fig.suptitle('Rebit Rust: Raw / Clean actual-engine spectra\n48 kHz, 32,768 coherent samples; FX bypassed')
fig.savefig(root/'spectra.png',dpi=150)
fig.savefig(root/'spectra.svg')
for shape in ['pulse125','pulse25','pulse50','triangle']:
    for hz in sorted({r['frequency_hz'] for r in results}):
        pair={r['mode']:r['alias_to_harmonic_db'] for r in results if r['shape']==shape and r['frequency_hz']==hz}
        print(f"{shape:9} {hz:8.1f} Hz: Raw {pair['raw']:7.2f} dB, Clean {pair['clean']:7.2f} dB, improvement {pair['raw']-pair['clean']:6.2f} dB")
