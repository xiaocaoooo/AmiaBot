/**
 * 用户信息管理类
 * 负责从 /api/self 接口获取用户信息并更新页面显示
 */
class UserInfoManager {
  // 定义用户信息的接口类型
  private userInfo: UserInfo | null = null;
  private avatarElement: HTMLImageElement | null = null;
  private nameElement: HTMLElement | null = null;
  private idElement: HTMLElement | null = null;

  /**
   * 构造函数，初始化用户信息管理器
   */
  constructor() {
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
  private async fetchUserInfo(): Promise<void> {
    try {
      const response = await fetch('/api/self');
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      this.userInfo = await response.json() as UserInfo;
      this.updateUserInfoDisplay();
    } catch (error) {
      console.error('Failed to fetch user info:', error);
      this.displayErrorMessage('获取用户信息失败');
    }
  }

  /**
   * 更新页面上的用户信息显示
   */
  private updateUserInfoDisplay(): void {
    if (!this.userInfo) return;

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
  private displayErrorMessage(message: string): void {
    console.log(message);
    // 可以在这里添加更多的错误处理逻辑，比如显示一个错误提示框
  }
}

/**
 * 用户信息接口类型定义
 */
interface UserInfo {
  qq: number;
  nick: string;
  remark: string;
  country: string;
  city: string;
  reg_time: number;
  qid: string;
  birthday: number;
  age: number;
  sex: 'male' | 'female' | 'unknown';
  avatar: string;
}

// 页面加载完成后初始化用户信息管理器
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => {
    new UserInfoManager();
  });
} else {
  // 如果DOM已经加载完成，则直接初始化
  new UserInfoManager();
}
