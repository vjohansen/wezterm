# `pane:get_program_status([id])`

{{since('nightly')}}

Returns a *program status* record associated with the pane, or `nil` if there
is no such record.

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

* `id` - the record's id, such as `"build/test"`; `nil` for the root record
* `state` - one of `"idle"`, `"working"`, `"done"`, `"blocked"` or `"error"`
* `kind` - what a `blocked` program is waiting on: `"permission"`,
  `"question"` or `"auth"`
* `progress` - a number from 0 to 100, for `working` or `blocked`
* `app` - a stable machine-readable name for the program, such as `cargo`.
  A child record without an `app` takes the `app` of its nearest ancestor.
* `title` - a short human-readable label (decoded from base64)
* `msg` - one human-readable line describing the state (decoded from base64)

`title` and `msg` come from the program, so treat them as untrusted text. A
report whose text contains control characters is ignored. Text direction
overrides and other invisible formatting characters are removed, so that the
text cannot appear different from what it is when shown in a tab title or
the status area.

### Records and ids

A program reports its own state using the *root* record, which has no id. A
program that does several things at once can also report *child* records,
each addressed by an `id` such as `us-east` or `build/test`; `build/test` is
a child of `build`. The pane holds all of these records at the same time, up
to 256 of them.

When *id* is omitted, returns the record that most urgently needs your
attention, in the order `blocked`, `error`, `working`, `done`, `idle`. Of
equally urgent records, the most recently updated one is returned. When a
program only reports the root record, this is the root record.

When *id* is `""`, returns the root record.

Otherwise, returns the child record whose id is *id*.

[pane:get_program_statuses()](get_program_statuses.md) returns every record.

### Lifetime

Each report replaces its record completely. A report with `state=clear`
removes the addressed record and every record beneath it, or every record
when it has no `id`. `working` and `blocked` records are removed when the
process attached to the pane exits, or when a new shell prompt begins
(`OSC 133 ; A`, emitted by [shell integration](../../../shell-integration.md)).
`idle`, `done` and `error` records remain until they are replaced, or until
you remove them with [pane:clear_program_status()](clear_program_status.md).
A full reset (`ESC c`) removes every record.

WezTerm replies to the `ESC ] 7501 ; ? ST` feature detection query.

When a record changes, WezTerm triggers an update of the tab bar, which will
in turn cause events such as
[format-tab-title](../window-events/format-tab-title.md) to trigger. The most
urgent record is also available as the `program_status` field of
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
