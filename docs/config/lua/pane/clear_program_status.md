# `pane:clear_program_status()`

{{since('nightly')}}

Removes the *program status* record associated with the pane, as if the
program had reported `state=clear`. See
[pane:get_program_status()](get_program_status.md).

`done` and `error` records are kept until they are replaced or cleared, so
that the result is still there when you come back to the pane. This example
clears them once the pane has been looked at, that is, when it is the active
pane of the focused window. `update-status` runs for the active pane of every
window, focused or not, every
[status_update_interval](../config/status_update_interval.md) milliseconds,
so the example checks
[window:is_focused()](../window/is_focused.md) first.

```lua
local wezterm = require 'wezterm'

wezterm.on('update-status', function(window, pane)
  if not window:is_focused() then
    return
  end
  local status = pane:get_program_status()
  if status and (status.state == 'done' or status.state == 'error') then
    pane:clear_program_status()
  end
end)

return wezterm.config_builder()
```

!!! note
    For a pane in a multiplexer domain, this only clears the copy of the
    record held by the GUI; the record in the remote terminal is unchanged,
    and is shown again when the program next reports its status.
