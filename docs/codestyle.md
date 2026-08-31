# Code style

## Naming

Prefer names that don't require the import path to be meaningful and unique. This allows us to use bare imports without negatively impacting readability.
E.g. prefer:
- `GuiCmd` over `gui::Cmd`
- `auto_connect_if_enabled()` over `auto_connect::run()`
