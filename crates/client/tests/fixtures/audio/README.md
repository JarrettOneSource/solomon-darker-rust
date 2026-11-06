# Native Opus fixtures

These synthesized tones contain no third-party recordings. FFmpeg 9.0.1 with
libopus encoded the short inputs at 48 kHz for exactly 0.137 seconds (6,576 frames).
They exercise encoder pre-skip and a final page that ends within an Opus packet.
The long fixture has 102,576 frames across multiple audio pages.

```sh
ffmpeg -f lavfi -i 'sine=frequency=431:sample_rate=48000:duration=0.137' -c:a libopus -b:a 64k -application audio opus-mono.ogg
ffmpeg -f lavfi -i 'aevalsrc=0.2*sin(2*PI*431*t)|0.15*sin(2*PI*853*t):s=48000:d=0.137' -c:a libopus -b:a 96k -application audio opus-stereo.ogg
ffmpeg -f lavfi -i 'aevalsrc=0.08*sin(2*PI*311*t)|0.08*sin(2*PI*401*t)|0.08*sin(2*PI*503*t)|0.08*sin(2*PI*607*t)|0.08*sin(2*PI*701*t)|0.08*sin(2*PI*809*t):s=48000:d=0.137:c=5.1' -c:a libopus -b:a 192k -application audio opus-surround.ogg
ffmpeg -f lavfi -i 'aevalsrc=0.2*sin(2*PI*431*t)|0.15*sin(2*PI*853*t):s=48000:d=2.137' -c:a libopus -b:a 96k -application audio opus-long.ogg
ffmpeg -request_sample_fmt flt -c:a libopus -i opus-surround.ogg -f f32le surround-oracle.f32
```

`opus-surround-oracle.json` records 11 decoded frame pairs, including both ends
and packet boundaries. FFmpeg emits FL, FR, FC, LFE, BL, BR. In that order, the
Xiph libopusfile stereo weights are `(0.529, 0)`, `(0, 0.529)`,
`(0.3741, 0.3741)`, `(0.3741, 0.3741)`, `(0.4582, 0.2645)` and
`(0.2645, 0.4582)`. Each recorded side is the sum of its six weighted float samples.
The regression allows 0.00001 for codec-build float differences. The tests also
rewrite headers through the Ogg writer for valid CRCs, long pre-skip and +6 dB gain;
concatenating the mono/stereo files exercises actual chained Ogg streams.

FFmpeg is a fixture-authoring tool, not a build or runtime dependency. The M2
fixture command used its already-installed x265 4.2 library through a process-local
`DYLD_LIBRARY_PATH`; no shared Homebrew installation was changed.
