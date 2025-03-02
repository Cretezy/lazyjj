## Configuring keybindings

```toml
# change keybinding
save = "ctrl+s"
# set multiple keybindings
save = ["ctrl+s", "ctrl+shift+g"]
# disable keybinding
save = false
```

In below examples default values are used.

### Log tab

```toml
[lazyjj.keybinds.log_tab]
save = "ctrl+s"
cancel = "esc"

close-popup = "q"

scroll-down = ["j", "down"]
scroll-up = ["k", "up"]
scroll-down-half = "shift+j"
scroll-up-half = "shift+k"

focus-current = "@"
toggle-diff-format = "w"

refresh = ["shift+r", "f5"]
create-new = "n"
create-new-describe = "shift+n"
squash = "s"
squash-ignore-immutable = "shift+s"
edit-change = "e"
edit-change-ignore-immutable = "shift+e"
abandon = "a"
describe = "d"
edit-revset = "r"
set-bookmark = "b"
open-files = "enter"

push = "p"
push-new = "ctrl+p"
push-all = "shift+p"
push-all-new = "ctrl+shift+p"
fetch = "f"
fetch-all = "shift+f"

open-help = "?"
```
