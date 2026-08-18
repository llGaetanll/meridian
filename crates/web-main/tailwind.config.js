const colors = require('tailwindcss/colors')

/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: 'class',
  content: [
    "../web-*/src/**/*.rs"
  ],
  theme: {
    extend: {
      colors: {
        accent: colors.lime,
        base: colors.slate,
      },
    },
  },
}
