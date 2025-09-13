"use strict";
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
/**
 * 用户信息管理类
 * 负责从 /api/self 接口获取用户信息并更新页面显示
 */
class UserInfoManager {
    /**
     * 构造函数，初始化用户信息管理器
     */
    constructor() {
        // 定义用户信息的接口类型
        this.userInfo = null;
        this.avatarElement = null;
        this.nameElement = null;
        this.idElement = null;
        // 获取页面元素引用
        this.avatarElement = document.querySelector('#user-avatar img');
        this.nameElement = document.getElementById('user-name');
        this.idElement = document.getElementById('user-id');
        // 页面加载时获取用户信息
        this.fetchUserInfo();
    }
    /**
     * 从 /api/self 接口获取用户信息
     */
    fetchUserInfo() {
        return __awaiter(this, void 0, void 0, function* () {
            try {
                const response = yield fetch('/api/self');
                if (!response.ok) {
                    throw new Error(`HTTP error! status: ${response.status}`);
                }
                this.userInfo = (yield response.json());
                this.updateUserInfoDisplay();
            }
            catch (error) {
                console.error('Failed to fetch user info:', error);
                this.displayErrorMessage('获取用户信息失败');
            }
        });
    }
    /**
     * 更新页面上的用户信息显示
     */
    updateUserInfoDisplay() {
        if (!this.userInfo)
            return;
        // 更新头像
        if (this.avatarElement && this.userInfo.avatar) {
            this.avatarElement.src = this.userInfo.avatar;
            this.avatarElement.alt = this.userInfo.nick || '用户头像';
        }
        // 更新用户名
        if (this.nameElement) {
            this.nameElement.textContent = this.userInfo.nick || '未知用户';
        }
        // 更新用户ID
        if (this.idElement) {
            this.idElement.textContent = `${this.userInfo.qq}`;
        }
    }
    /**
     * 显示错误信息
     * @param message 错误信息
     */
    displayErrorMessage(message) {
        console.log(message);
        // 可以在这里添加更多的错误处理逻辑，比如显示一个错误提示框
    }
}
// 页面加载完成后初始化用户信息管理器
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        new UserInfoManager();
    });
}
else {
    // 如果DOM已经加载完成，则直接初始化
    new UserInfoManager();
}
//# sourceMappingURL=base-info.js.map