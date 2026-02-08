# 🍏 SF Symbols for TypeScript

```ts
import type { SFSymbol } from 'sf-symbols-typescript'

const icon: SFSymbol = 'arrow.up'
```

<img style="border-radius: 8px; overflow: hidden; margin:0;" width="1021" alt="Screenshot 2023-03-03 at 3 21 13 PM" src="https://user-images.githubusercontent.com/13172299/222821298-0c0a4fba-fe0a-4e31-85d7-55e7870f2bc7.png" />

A simple TypeScript utility to get the names of SF Symbols. Mainly useful for React Native libraries and apps that utilize SF Symbols.

I made this for [Zeego](https://zeego.dev) and [Burnt](https://github.com/nandorojo/burnt).

It has zero dependencies, and zero runtime [code](https://github.com/nandorojo/sf-symbols-typescript/blob/master/dist/index.js).

## Installation

```sh
yarn add sf-symbols-typescript
```

## Usage

```ts
import type { SFSymbol } from 'sf-symbols-typescript'

const icon: SFSymbol = 'arrow.up'
```

## Restricting symbols

By default, this library exposes all available symbols for the current version of SF Symbols.

Depending on the platforms your app targets, you may want to restrict the types to specific versions of SF Symbols.

### Globally

You can restrict the `SFSymbol` type to a specific version of SF Symbols through declaration merging.

This library exposes an interface called `Overrides` with a property `SFSymbolsVersion` that can be overidden.

```ts
declare module 'sf-symbols-typescript' {
  interface Overrides {
    // globally restrict symbols to those found in SF Symbols 4.2
    SFSymbolsVersion: '4.2'
  }
}
```

### Individually

If you require more granular control, this library also exposes individual type definitions for each version of SF Symbols.

They are named with this pattern: `SFSymbols{major}_{minor}`

```ts
import type { SFSymbols4_2, SFSymbols5_0 } from 'sf-symbols-typescript'
```

### Version Support Table

Here are the platforms supported by each version of SF Symbols:

| SFSymbols | iOS  | macOS | tvOS | visionOS | watchOS |
| --------- |------|-------|------|----------|---------|
| **1.0**   | 13.0 | 11.0  | 13.0 | 1.0      | 6.0     |
| **1.1**   | 13.1 | 11.0  | 13.0 | 1.0      | 6.1     |
| **2.0**   | 14.0 | 11.0  | 14.0 | 1.0      | 7.0     |
| **2.1**   | 14.2 | 11.0  | 14.2 | 1.0      | 7.1     |
| **2.2**   | 14.5 | 11.3  | 14.5 | 1.0      | 7.4     |
| **3.0**   | 15.0 | 12.0  | 15.0 | 1.0      | 8.0     |
| **3.1**   | 15.1 | 12.0  | 15.1 | 1.0      | 8.1     |
| **3.2**   | 15.2 | 12.1  | 15.2 | 1.0      | 8.3     |
| **3.3**   | 15.4 | 12.3  | 15.4 | 1.0      | 8.5     |
| **4.0**   | 16.0 | 13.0  | 16.0 | 1.0      | 9.0     |
| **4.1**   | 16.1 | 13.0  | 16.1 | 1.0      | 9.1     |
| **4.2**   | 16.4 | 13.3  | 16.4 | 1.0      | 9.4     |
| **5.0**   | 17.0 | 14.0  | 17.0 | 1.0      | 10.0    |
| **5.1**   | 17.2 | 14.2  | 17.2 | 1.1      | 10.2    |
| **5.2**   | 17.4 | 14.24 | 17.4 | 1.1      | 10.4    |
| **5.3**   | 17.6 | 14.6  | 17.6 | 1.3      | 10.6    |
| **6.0**   | 18.0 | 15.0  | 18.0 | 2.0      | 11.0    |
| **6.1**   | 18.1 | 15.1  | 18.1 | 2.1      | 11.1    |
| **6.2**   | 18.2 | 15.2  | 18.2 | 2.2      | 11.2    |
| **6.3**   | 18.4 | 15.4  | 18.4 | 2.3      | 11.4    |
| **6.4**   | 18.5 | 15.5  | 18.5 | 2.4      | 11.5    |
| **7.0**   | 26.0 | 26.0  | 26.0 | 26.0     | 26.0    |

## Contributing

To update this package's icons, you will need [Bun](https://bun.sh/) installed on your machine.

You will also need a copy of the plist resources from the [SF Symbols desktop application](https://developer.apple.com/sf-symbols/).

- Right click the SF Symbols application in Finder and choose "Show Package Contents"
- Navigate to `./Contents/Resources/Metadata`. The required plist resources are here.

Alternatively, you may be able to find those same resource files in the source code of the unofficial [SFSafeSymbols Swift library](https://github.com/SFSafeSymbols/SFSafeSymbols/tree/stable/SymbolsGenerator/Sources/SymbolsGenerator/Resources).

1. Clone this repo
1. Copy the SF Symbols plist resource files into the `./src/data` directory of the repo
1. Run `bun run build`
1. Push your changes and open a PR

## Disclaimer

It is your responsibility to check Apple's rules about when certain icons can be used. You can check [the SF Symbols desktop application](https://developer.apple.com/sf-symbols/) for more info.

I am purposefully not displaying any icons themselves to avoid violating any licenses. This library isn't associated with Apple, and is only intended to expose the _names_ of the icons themselves.

## Credits

This project is based on Apple's [SF Symbols](https://developer.apple.com/design/human-interface-guidelines/sf-symbols/overview/)

Previous versions of this library used data sourced from the now-defunct SFSymbols.com. Thanks to [Noah Sark](https://github.com/noahsark769/SFSymbols.com) for your open source contributions.

## License

MIT License
