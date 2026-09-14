# simple. usually use this tools.

```bash
# for gif
gifski -o cosmostrix-video.gif --fps 60 --quality 100 cosmostrix-video.webm

# for webp
ffmpeg -i cosmostrix-video.webm -c:v libwebp_anim -vf "fps=20,scale=960:-1:flags=lanczos" -quality 80 -compression_level 6 -loop 0 -an cosmostrix-video.webp
```
