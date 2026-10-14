# `pane:get_program_statuses()`

{{since('nightly')}}

Returns an array of every *program status* record associated with the pane:
the root record first, then the child records ordered by id. Each entry has
the fields described in [pane:get_program_status()](get_program_status.md),
including `id`, which is `nil` for the root record. The array is empty if
there are no records.

Use it to present several records, or to pick a record using your own
order instead of the one that `pane:get_program_status()` uses. This example
counts the records in each state, giving a summary such as `?1 …2`:

```lua
local wezterm = require 'wezterm'

local SYMBOLS = {
  { 'blocked', '?' },
  { 'error', '✘' },
  { 'working', '…' },
  { 'done', '✔' },
}

wezterm.on(
  'format-tab-title',
  function(tab, tabs, panes, config, hover, max_width)
    local pane = wezterm.mux.get_pane(tab.active_pane.pane_id)
    local counts = {}
    for _, status in ipairs(pane:get_program_statuses()) do
      counts[status.state] = (counts[status.state] or 0) + 1
    end
    local summary = ''
    for _, entry in ipairs(SYMBOLS) do
      local state, symbol = table.unpack(entry)
      if counts[state] then
        summary = summary .. symbol .. counts[state] .. ' '
      end
    end
    return ' ' .. summary .. tab.active_pane.title .. ' '
  end
)

return wezterm.config_builder()
```
