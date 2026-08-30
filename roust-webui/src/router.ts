import { createRouter, createWebHistory } from 'vue-router'
import StatusPage from '@/pages/StatusPage.vue'
import RoutesPage from '@/pages/RoutesPage.vue'
import SettingsPage from '@/pages/SettingsPage.vue'
import AboutMePage from '@/pages/AboutMePage.vue'

const base = import.meta.env.BASE_URL || '/'

export const router = createRouter({
  history: createWebHistory(base),
  routes: [
    { path: '/', redirect: '/status' },
    { path: '/status', name: 'status', component: StatusPage },
    { path: '/routes', name: 'routes', component: RoutesPage },
    { path: '/settings', name: 'settings', component: SettingsPage },
    { path: '/about-me', name: 'about-me', component: AboutMePage },
  ],
})
