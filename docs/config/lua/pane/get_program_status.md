# `pane:get_program_status()`

{{since('nightly')}}

Returns the *program status* record associated with the pane, or `nil` if no
program has reported one.

A program reports what it is doing using the [Program Status
Protocol](https://www.superlogical.com/rex/docs/build/program-status) OSC
sequence:

```
ESC ] 7501 ; key=value:key=value... ST
```

For example, terraform waiting for the user to approve an apply might send:

```
ESC ] 7501 ; state=blocked:kind=permission:app=terraform:msg=QXBwbHkgMyB0byBhZGQsIDEgdG8gY2hhbmdlLCAwIHRvIGRlc3Ryb3k/ ST
```

which is returned as:

```lua
{
  state = 'blocked',
  kind = 'permission',
  app = 'terraform',
  msg = 'Apply 3 to add, 1 to change, 0 to destroy?',
}
```

The returned table has the following fields; all but `state` may be `nil`:

* `state` - one of `"idle"`, `"working"`, `"done"`, `"blocked"` or `"error"`
* `kind` - what a `blocked` program is waiting on: `"permission"`,
  `"question"` or `"auth"`
* `progress` - a number from 0 to 100, for `working` or `blocked`
* `app` - a stable machine-readable name for the program, such as `cargo`
* `title` - a short human-readable label (decoded from base64)
* `msg` - one human-readable line describing the state (decoded from base64)

Each report replaces the record completely. A report with `state=clear`
removes it. `working` and `blocked` records are removed when the process
attached to the pane exits, or when a new shell prompt begins (`OSC 133 ; A`,
emitted by [shell integration](../../../shell-integration.md)). `idle`, `done`
and `error` records remain until they are replaced, or until you remove them
with [pane:clear_program_status()](clear_program_status.md).

WezTerm only tracks the root record: reports that carry an `id` are ignored.
WezTerm replies to the `ESC ] 7501 ; ? ST` feature detection query.

When the record changes, WezTerm triggers an update of the tab bar, which will
in turn cause events such as
[format-tab-title](../window-events/format-tab-title.md) to trigger. The
record is also available as the `program_status` field of
[PaneInformation](../PaneInformation.md).

The program status is not directly used by wezterm, but you can use it to
style tabs. This example prefixes the tab title with a symbol and color
for the state:

```lua
local wezterm = require 'wezterm'

local STATUS = {
  idle = { '·', 'grey' },
  working = { '…', 'skyblue' },
  done = { '✔', 'lime' },
  blocked = { '?', 'orange' },
  error = { '✘', 'red' },
}

wezterm.on(
  'format-tab-title',
  function(tab, tabs, panes, config, hover, max_width)
    local title = tab.active_pane.title
    local status = tab.active_pane.program_status
    if status == nil then
      return ' ' .. title .. ' '
    end
    local symbol, color = table.unpack(STATUS[status.state])
    if status.progress then
      symbol = symbol .. status.progress .. '%'
    end
    return {
      { Foreground = { Color = color } },
      { Text = ' ' .. symbol },
      { Foreground = 'Default' },
      { Text = ' ' .. title .. ' ' },
    }
  end
)

return wezterm.config_builder()
```
