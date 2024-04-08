import { defineConfig } from 'vitepress'

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "yabruc",
  description: "A bru cli clone written in Rust!",
  base: '/yabruc/',
  head: [['link', { rel: 'icon', href: '/yabruc/yabruc.ico' }]],
  themeConfig: {
    logo: '/yabruc.webp',
    // https://vitepress.dev/reference/default-theme-config
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Docs', link: '/usage' }
    ],

    sidebar: [
      {
        text: 'Docs',
        items: [
          { text: 'Usage', link: '/usage' },
          {
            text: 'Compatibility', items: [
              { text: 'General compatibility table', link: '/compatibility/' },
              { text: 'HTTP', link: '/compatibility/HTTP' },
            ]
          },
          { text: 'How it works', link: '/how-it-works' }
        ]
      }
    ],

    socialLinks: [
      { icon: 'github', link: 'https://github.com/aitorru/yabruc' }
    ]
  }
})
