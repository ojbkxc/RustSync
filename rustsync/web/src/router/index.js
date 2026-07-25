import { createRouter, createWebHashHistory } from "vue-router";
import layout from "@/views/layout.vue";
import { useAppStore } from "@/store/useAppStore";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: "/",
      component: () => import("@/views/index.vue"),
    },
    {
      path: "/login",
      component: () => import("@/views/login.vue"),
    },
    {
      path: "/home",
      component: layout,
      children: [
        {
          path: "",
          component: () => import("@/views/pages/home/index.vue"),
          meta: {
            leftIndex: "/home",
          },
        },
        {
          path: "task",
          component: () => import("@/views/pages/home/task.vue"),
          meta: {
            leftIndex: "/home",
          },
        },
        {
          path: "task/detail",
          component: () => import("@/views/pages/home/taskDetail.vue"),
          meta: {
            leftIndex: "/home",
          },
        },
      ],
    },
    {
      path: "/engine",
      component: layout,
      children: [
        {
          path: "",
          component: () => import("@/views/pages/engine/index.vue"),
          meta: {
            leftIndex: "/engine",
          },
        },
      ],
    },
    {
      path: "/notify",
      component: layout,
      children: [
        {
          path: "",
          component: () => import("@/views/pages/notify/index.vue"),
          meta: {
            leftIndex: "/notify",
          },
        },
      ],
    },
    {
      path: "/setting",
      component: layout,
      children: [
        {
          path: "",
          component: () => import("@/views/pages/setting/index.vue"),
          meta: {
            leftIndex: "/setting",
          },
        },
      ],
    },
  ],
});

// 路由守卫：检查登录状态
router.beforeEach((to, from, next) => {
  const appStore = useAppStore();
  // 需要认证的路由（有 meta.leftIndex 标记）
  if (to.meta.leftIndex && !appStore.user) {
    next("/login");
    return;
  }
  // 已登录用户访问登录页，重定向到首页
  if (to.path === "/login" && appStore.user) {
    next("/home");
    return;
  }
  next();
});

export default router;
