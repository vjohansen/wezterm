# `pane:clear_program_status([id])`

{{since('nightly')}}

Removes *program status* records associated with the pane. See
[pane:get_program_status()](get_program_status.md).

When *id* is omitted, removes every record.

When *id* is `""`, removes just the root record. Otherwise, removes just the
child record whose id is *id*. Unlike a `state=clear` report from the
program, clearing a single record keeps the records beneath it.

`done` and `error` records are kept until they are replaced or cleared, so
that the result is still there when you come back to the pane. This example
clears them once the pane has been looked at, that is, when it is the active
pane of the focused window. `update-status` runs for the active pane of every
window, focused or not, every
[status_update_interval](../config/status_update_interval.md) milliseconds,
so the example checks
[window:is_focused()](../window/is_focused.md) first. It removes the `done`
and `error` records one by one, so that records that are still `working` or
`blocked` are kept.

```lua
local wezterm = require 'wezterm'

wezterm.on('update-status', function(window, pane)
  if not window:is_focused() then
    return
  end
  for _, status in ipairs(pane:get_program_statuses()) do
    if status.state == 'done' or status.state == 'error' then
      pane:clear_program_status(status.id or '')
    end
  end
end)

return wezterm.config_builder()
```

!!! note
    For a pane in a multiplexer domain, the records are held by the
    multiplexer server, so clearing them there also clears them for every
    other client attached to the pane.
