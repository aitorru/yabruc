import { defineConfig } from 'vitepress'

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "yabruc",
  description: "A bru cli clone written in Rust!",
  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Docs', link: '/docs' }
    ],

    sidebar: [
      {
        text: 'Docs',
        items: [
          { text: 'Usage', link: '/usage' },
          { text: 'How it works', link: '/how-it-works' }
        ]
      }
    ],

    socialLinks: [
      { icon: 'github', link: 'https://github.com/aitorru/yabruc' }
    ]
  }
})
