import { defineStore } from 'pinia'

export const useAppStore = defineStore('app', {
  state: () => ({
    user: null,
    leftIndex: '/home',
    loading: false,
    onRequest: false,
    cookieName: 'rust_sync',
  }),

  actions: {
    set(key, value) {
      this[key] = value
    },
  },

  persist: {
    storage: localStorage,
    pick: ['user'],
  },
})