# Tatum

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
  vim.fn.jobstart({"tatum", "serve", "--open", vim.fn.expand('%')}, { noremap = true, silent = true })
end)
```

## Features

Tatum aims to make entirely self-contained `HTML` files.
If you reference an image in your Markdown, Tatum will resolve the location of the image, encode it as a data URL, and place it in the final file.
