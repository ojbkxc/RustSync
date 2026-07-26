<template>
    <div class="logo">
        <el-image style="width: 36px; height: 36px" src="/logo.png" fit="contain" />
        <div class="logo-title">{{ $t('title') }}</div>
    </div>
</template>

<style lang="scss" scoped>
.logo {
    display: flex;
    align-items: center;

    .logo-title {
        font-family: "Poppins", "微软雅黑", sans-serif;
        font-size: 24px;
        font-weight: bold;
        margin-left: 12px;
        background: linear-gradient(135deg,
                #3b72f0 0%,
                #6094f8 25%,
                #22d3ee 50%,
                #6094f8 75%,
                #3b72f0 100%);
        background-clip: text;
        -webkit-text-fill-color: transparent;
        animation: shine 6s linear infinite;
        background-size: 200% auto;
    }

    @keyframes shine {
        to {
            background-position: 200% center;
        }
    }
}
</style>
