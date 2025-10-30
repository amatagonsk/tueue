# tueue

tueue is tui pueue status monitor.

<img src="https://raw.githubusercontent.com/amatagonsk/tueue/master/img/tueue.avif" width="100%" />

(if not animated image, youtube link [here](https://youtu.be/wGAfdNO8TKk))

## features

every 5sec interval run command `pueue status $input_args`, and show result.
similar to `watch` command.

## required

- pueue (daemon running)

## installation
### crate.io
```
cargo install tueue
```

### github
```
cargo install --git https://github.com/amatagonsk/tueue.git
```

or download from [release](https://github.com/amatagonsk/tueue/releases)


## additional hotkey

`PageUp/PageDown`: scroll up/down 20  
`Home/End`: scroll left/right 20  
`ctrl+wheel up/down`: scroll left/right  




## nice tools

[pueue](https://github.com/Nukesor/pueue)  

[ratatui](https://github.com/ratatui-org/ratatui)  