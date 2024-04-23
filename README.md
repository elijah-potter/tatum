## Tatum

Tatum is yet another small tool for rendering Markdown to HTML.
Primarily, I use it to preview my Markdown notes and essays, and to export them to self-contained HTML files.

In my case, this is for submission to school assignments.

You my replicate my setup by first installing Tatum:

```bash
cargo install --git https://github.com/elijah-potter/tatum --locked
```

Next, insert the following snippet into your Neovim config:

```lua
vim.keymap.set("n", "<leader>o", function ()
  function handle_stdout(chan_id, data, name)
    data = data[1]

    os.execute("xdg-open http://" .. data .. "?path=" .. vim.fn.expand('%'))
  end

  vim.fn.jobstart('tatum serve -q', { on_stdout = handle_stdout })
end, { noremap = true, silent = true })
```
