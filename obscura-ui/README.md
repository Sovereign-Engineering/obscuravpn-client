# App UI for Obscura VPN

## Libraries

- [React Icons](https://react-icons.github.io/react-icons)
- [Mantine Docs](https://mantine.dev/pages/basics/)
- [Mantine Default Theme](https://github.com/mantinedev/mantine/blob/master/src/mantine-styles/src/theme/default-theme.ts)
- [react-18next Trans Component](https://react.i18next.com/latest/trans-component)

## Tips and Trouble Shooting

- Broken npm sub-dependency? Use `resolutions: {subDependency: version}`
- Use `pnpm upgrade --interactive` to upgrade package interactively
  - use `npm install --package-lock-only` to update `package-lock.json` which is used to generate the license.json used by the UI

### Media Queries

For adding new mobile styles, you can do the following

```css
@media screen and (max-width: $mantine-breakpoint-xs) {
    padding-top: env(safe-area-inset-top) !important;
}
```
