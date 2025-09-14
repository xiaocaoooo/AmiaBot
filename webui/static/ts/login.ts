/**
 * 登录页面的客户端逻辑
 */
class LoginManager {
  private loginForm: HTMLFormElement | null;
  private passwordInput: HTMLInputElement | null;
  
  /**
   * 构造函数，初始化登录管理器
   */
  constructor() {
    this.loginForm = document.querySelector('form');
    this.passwordInput = document.getElementById('password') as HTMLInputElement | null;
    this.initEventListeners();
  }
  
  /**
   * 初始化事件监听器
   */
  private initEventListeners(): void {
    // 表单提交事件
    if (this.loginForm) {
      this.loginForm.addEventListener('submit', (event: Event) => {
        // 阻止默认提交，改用自定义提交方式
        event.preventDefault();
        this.submitWithHashedPassword();
      });
    }
    
    // 密码输入框键盘事件
    if (this.passwordInput) {
      this.passwordInput.addEventListener('keydown', (event: KeyboardEvent) => {
        // 按下Enter键时自动提交表单
        if (event.key === 'Enter') {
          if (this.validateForm() && this.loginForm) {
            this.loginForm.dispatchEvent(new Event('submit'));
          }
        }
      });
      
      // 输入时清除错误信息
      this.passwordInput.addEventListener('input', () => {
        const errorMessage = document.querySelector('.error-message');
        if (errorMessage) {
          errorMessage.remove();
        }
      });
    }
  }
  
  /**
   * 验证表单
   * @returns 验证是否通过
   */
  private validateForm(): boolean {
    if (!this.passwordInput || !this.passwordInput.value.trim()) {
      this.showError('请输入密码');
      return false;
    }
    return true;
  }
  
  /**
   * 显示错误信息
   * @param message 错误信息
   */
  private showError(message: string): void {
    // 移除已存在的错误信息
    const existingError = document.querySelector('.error-message');
    if (existingError) {
      existingError.remove();
    }
    
    // 创建新的错误信息元素
    const errorElement = document.createElement('div');
    errorElement.className = 'error-message';
    errorElement.textContent = message;
    
    // 添加到表单之前
    if (this.loginForm && this.loginForm.parentNode) {
      this.loginForm.parentNode.insertBefore(errorElement, this.loginForm);
    }
  }
  
  /**
   * 计算字符串的SHA-256哈希值
   * @param str 要哈希的字符串
   * @returns 十六进制格式的哈希值
   */
  private async calculateSHA256(str: string): Promise<string> {
    try {
      // 将字符串转换为ArrayBuffer
      const encoder = new TextEncoder();
      const data = encoder.encode(str);
      
      // 计算SHA-256哈希值
      const hashBuffer = await crypto.subtle.digest('SHA-256', data);
      
      // 将ArrayBuffer转换为十六进制字符串
      const hashArray = Array.from(new Uint8Array(hashBuffer));
      const hashHex = hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
      
      return hashHex;
    } catch (error) {
      console.error('计算哈希值失败:', error);
      throw error;
    }
  }
  
  /**
   * 使用哈希密码提交表单
   */
  private async submitWithHashedPassword(): Promise<void> {
    if (!this.validateForm() || !this.passwordInput || !this.loginForm) {
      return;
    }
    
    try {
      // 获取原始密码
      const password = this.passwordInput.value;
      
      // 计算密码的哈希值
      const hashedPassword = await this.calculateSHA256(password);
      
      // 创建隐藏字段来存储哈希密码
      let hashedPasswordInput = this.loginForm.querySelector('input[name="hashed_password"]') as HTMLInputElement | null;
      if (!hashedPasswordInput) {
        hashedPasswordInput = document.createElement('input');
        hashedPasswordInput.type = 'hidden';
        hashedPasswordInput.name = 'hashed_password';
        this.loginForm.appendChild(hashedPasswordInput);
      }
      
      // 设置哈希密码值
      hashedPasswordInput.value = hashedPassword;
      
      // 清空原始密码输入框，避免在网络请求中发送原始密码
      const originalPassword = this.passwordInput.value;
      this.passwordInput.value = '';
      
      // 提交表单
      this.loginForm.submit();
    } catch (error) {
      console.error('提交表单失败:', error);
      this.showError('登录处理失败，请刷新页面重试');
      // 恢复密码输入框（如果用户需要重试）
      if (this.passwordInput) {
        this.passwordInput.value = '';
      }
    }
  }
}

/**
 * 初始化登录页面
 */
function initLoginPage(): void {
  try {
    // 创建登录管理器实例
    const loginManager = new LoginManager();
  } catch (error) {
    console.error('初始化登录页面失败:', error);
  }
}

/**
 * 页面加载完成后初始化
 */
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', initLoginPage);
} else {
  // 页面已经加载完成，直接初始化
  initLoginPage();
}
