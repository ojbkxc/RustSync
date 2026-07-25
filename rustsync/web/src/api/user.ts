import request from '@/utils/request'
import type { ApiResponse, LoginRequest, ResetPasswordRequest, ChangePasswordRequest, UserInfo, LoginResponse } from './types'

// POST /api/auth/login
export function login(data: LoginRequest) {
  return request<ApiResponse<LoginResponse>>({ url: '/api/auth/login', method: 'post', data })
}

// DELETE /api/auth/logout
export function logout() {
  return request<ApiResponse<null>>({ url: '/api/auth/logout', method: 'delete' })
}

// PUT /api/auth/reset-password
export function resetPassword(data: ResetPasswordRequest) {
  return request<ApiResponse<{ passwd?: string }>>({ url: '/api/auth/reset-password', method: 'put', data })
}

// GET /api/user
export function getUser() {
  return request<ApiResponse<UserInfo>>({ url: '/api/user', method: 'get' })
}

// PUT /api/user/password
export function changePassword(data: ChangePasswordRequest) {
  return request<ApiResponse<null>>({ url: '/api/user/password', method: 'put', data })
}