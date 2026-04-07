# DTXMania-rs

A prototype of DTXMania re-written using [Bevy](https://bevy.org/), based on the
[DTXManiaNX](https://github.com/limyz/DTXmaniaNX) fork.

This is mainly to satisfy my Linux need.

## Additional requirements

Aside from Rust, this app uses FFmpeg for media decoding so FFmpeg is required for
building and running. See <https://github.com/zmwangx/rust-ffmpeg/wiki/Notes-on-building>.

## TODO

- gameplay
  - [ ] bjxa
  - [ ] input hit detection
  - [ ] hit range (perfect, poor, etc.)
  - [ ] group lanes (HHC+HHO, etc.)
  - [ ] scores.ini
- song select
  - [ ] chart's metadata
  - [ ] song select tree
  - [ ] parallelize song scan
- ui/ux
  - [ ] config.ini
  - [ ] graphics visual
  - [ ] video
- optimization
  - [ ] song.db cache
