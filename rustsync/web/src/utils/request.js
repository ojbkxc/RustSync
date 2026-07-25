import axios from 'axios'
import { ElMessage } from 'element-plus'
import { useAppStore } from '@/store/useAppStore'
import router from '@/router'

const service = axios.create({
  baseURL: '',
  timeout: 30000,
  withCredentials: true,
})

// 请求拦截器
service.interceptors.request.use(
  (config) => {
    // 不需要额外处理，cookie 会自动携带
    return config
  },
  (error) => Promise.reject(error)
)

// 响应拦截器
service.interceptors.response.use(
  (response) => {
    const res = response.data
    if (res.code !== 200) {
      ElMessage.error(res.msg || '请求失败')
      if (res.code === 401) {
        const appStore = useAppStore()
        appStore.set('user', null)
        router.push('/login')
      }
      return Promise.reject(new Error(res.msg || '请求失败'))
    }
    return res
  },
  (error) => {
    ElMessage.error(error.message || '网络错误')
    return Promise.reject(error)
  }
)

export default service