/**
 * 导航菜单管理类
 * 负责处理页面导航、切换和状态更新
 */
class NavigationManager {
  private navLinks: NodeListOf<HTMLAnchorElement>;
  private page: HTMLElement | null;
  private pageNameElement: HTMLElement | null;

  /**
   * 构造函数，初始化导航管理器
   */
  constructor() {
    this.navLinks = document.querySelectorAll('#nav a');
    this.page = document.querySelector('#content #page');
    this.pageNameElement = document.getElementById('page-name');
    this.initEventListeners();
    // 初始化时根据当前URL路径导航到对应页面
    this.handleUrlChange();
  }

  /**
   * 初始化事件监听器
   */
  private initEventListeners(): void {
    // 导航链接点击事件
    this.navLinks.forEach(link => {
      link.addEventListener('click', (event: MouseEvent) => {
        event.preventDefault();
        const targetId = link.getAttribute('data-id') || '';

        // Reload the entire page to ensure a fresh state
        window.location.href = `${window.location.origin}${window.location.pathname.split('/').slice(0, -1).join('/')}/${targetId}`;
      });
    });

    // 监听URL变化事件（浏览器前进/后退按钮或直接修改URL）
    window.addEventListener('popstate', () => {
      this.handleUrlChange();
    });
  }

  /**
   * 处理URL变化事件
   */
  private handleUrlChange(): void {
    // 从URL路径中获取页面ID
    const pathParts = window.location.pathname.split('/').filter(Boolean);
    // 假设URL格式为 /webui/page-name 或 /page-name
    let pageId = 'base-info';

    if (pathParts.length >= 2) {
      pageId = pathParts[1];
    }

    this.navigateTo(pageId, false);
  }

  /**
   * 导航到指定页面
   * @param targetId 目标页面的ID
   * @param updateHistory 是否更新浏览器历史记录
   */
  private navigateTo(targetId: string, updateHistory: boolean = true): void {
    // 更新页面标题
    if (this.pageNameElement) {
      const linkText = Array.from(this.navLinks).find(link =>
        link.getAttribute('href')?.includes(targetId) || link.getAttribute('data-id') === targetId
      )?.textContent;
      this.pageNameElement.textContent = linkText || '未知页面';
    }

    // 如果需要更新历史记录，则添加到浏览器历史
    if (updateHistory) {
      const basePath = window.location.pathname.split('/').slice(0, -1).join('/') || '';
      history.pushState(null, '', `${basePath}/${targetId}`);
    }

    // 高亮当前活动的导航链接
    this.navLinks.forEach(link => {
      const linkTargetId = link.getAttribute('data-id') || '';
      if (linkTargetId === targetId) {
        link.classList.add('active');
      } else {
        link.classList.remove('active');
      }
    });

    // Request HTML content based on the target ID and add it to the page element
    if (this.page) {
      const url = `/static/pages/${targetId}.html`;
      fetch(url)
        .then(response => {
          if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
          }
          return response.text();
        })
        .then(html => {
          if (this.page) {
            // 清空页面内容
            this.page.innerHTML = '';

            // 创建临时容器来解析HTML
            const tempContainer = document.createElement('div');
            tempContainer.innerHTML = html;

            // 提取所有script标签
            const scripts = Array.from(tempContainer.querySelectorAll('script'));

            // 移除所有script标签，避免重复执行
            scripts.forEach(script => script.remove());

            // 先将所有非script内容添加到页面
            while (tempContainer.firstChild) {
              this.page.appendChild(tempContainer.firstChild);
            }

            // 重新创建并执行script标签
            scripts.forEach(script => {
              const newScript = document.createElement('script');

              // 复制所有属性
              Array.from(script.attributes).forEach(attr => {
                newScript.setAttribute(attr.name, attr.value);
              });

              // 复制内联脚本内容
              if (script.textContent) {
                newScript.textContent = script.textContent;
              }

              // 添加到页面以执行
              document.body.appendChild(newScript);

              // 执行后移除，避免累积
              newScript.onload = () => {
                document.body.removeChild(newScript);
              };
            });
          }
        })
        .catch(error => {
          console.error('Error loading page content:', error);
          if (this.page)
            this.page.innerHTML = `<p>Failed to load page: ${targetId}</p>`;
        });
    }
  }
}

/**
 * 弹窗管理类
 * 负责处理页面弹窗的显示、隐藏和交互
 */
class AlertManager {
  private alertsContainer: HTMLElement | null;

  /**
   * 构造函数，初始化弹窗管理器
   */
  constructor() {
    this.alertsContainer = document.getElementById('alerts-container');
    this.initEventListeners();
  }

  /**
   * 初始化事件监听器
   */
  private initEventListeners(): void {
    // 监听整个弹窗容器的点击事件，使用事件委托处理关闭按钮点击
    if (this.alertsContainer) {
      this.alertsContainer.addEventListener('click', (event: MouseEvent) => {
        const target = event.target as HTMLElement;
        if (target.closest('.alert-close')) {
          const alertContainer = target.closest('.alert-container');
          if (alertContainer) {
            this.removeAlert(alertContainer as HTMLElement);
          }
        }
        // 处理带有data-dismiss="alert"属性的元素点击事件
        else if (target.closest('[data-dismiss="alert"]')) {
          const alertContainer = target.closest('.alert-container');
          if (alertContainer) {
            this.removeAlert(alertContainer as HTMLElement);
          }
        }
        // Handle elements with data-action attribute
        const actionElement = target.closest('[data-action]');
        if (actionElement) {
          const action = actionElement.getAttribute('data-action');
          if (action) {
            const alertContainer = target.closest('.alert-container');
            if (alertContainer) {
              // Trigger a custom event based on the action, allowing the caller to handle the logic
              const actionEvent = new CustomEvent(`alert-${action}`, {
                bubbles: true,
                detail: {
                  alertElement: alertContainer
                }
              });
              alertContainer.dispatchEvent(actionEvent);
            }
          }
        }
        // 点击遮罩层（非弹窗内容区域）关闭弹窗
        else if (target === this.alertsContainer) {
          // 获取当前显示的弹窗
          const alertElement = this.alertsContainer.querySelector('.alert-container');
          if (alertElement) {
            this.removeAlert(alertElement as HTMLElement);
          }
        }
      });
    }

    // 添加键盘事件监听，支持Esc键关闭弹窗
    document.addEventListener('keydown', (event: KeyboardEvent) => {
      // 当按下Esc键时关闭当前弹窗
      if (event.key === 'Escape') {
        const container = this.alertsContainer;
        if (container) {
          const alertElement = container.querySelector('.alert-container');
          if (alertElement) {
            this.removeAlert(alertElement as HTMLElement);
          }
        }
      }
    });
  }

  /**
   * 显示弹窗
   * @param title 弹窗标题 (支持字符串或HTML元素)
   * @param content 弹窗内容 (支持字符串或HTML元素)
   * @param type 弹窗类型 (默认: 'info')
   * @param autoClose 是否自动关闭 (默认: false)
   * @param duration 自动关闭延迟时间(毫秒) (默认: 3000)
   * @returns 创建的弹窗元素
   */
  public showAlert(
    title: string | HTMLElement,
    content: string | HTMLElement,
    type: 'info' | 'success' | 'warning' | 'error' = 'info',
    autoClose: boolean = false,
    duration: number = 3000
  ): HTMLElement {
    // 获取或创建弹窗容器
    const container = this.getAlertsContainer();
    if (!container) {
      console.error('Alerts container not found or could not be created');
      // 创建一个临时元素并返回，避免返回null
      const tempDiv = document.createElement('div');
      return tempDiv;
    }

    // 创建弹窗元素
    const alertElement = document.createElement('div');
    // 默认使用secondary-container样式
    alertElement.className = `alert-container secondary-container`;

    // 创建标题和内容容器
    const titleElement = document.createElement('div');
    titleElement.className = 'alert-title';

    const closeElement = document.createElement('div');
    closeElement.className = 'alert-close';
    closeElement.innerHTML = `<i class="fas fa-times"></i>`;

    const contentElement = document.createElement('div');
    contentElement.className = 'alert-content';

    // 设置弹窗标题
    if (typeof title === 'string') {
      titleElement.innerHTML = title;
    } else {
      titleElement.appendChild(title);
    }

    // 设置弹窗内容
    if (typeof content === 'string') {
      contentElement.innerHTML = content;
    } else {
      contentElement.appendChild(content);
    }

    // 将元素添加到弹窗中
    alertElement.appendChild(titleElement);
    alertElement.appendChild(closeElement);
    alertElement.appendChild(contentElement);

    // 添加到容器
    container.appendChild(alertElement);

    // 如果设置了自动关闭，则在指定时间后关闭
    if (autoClose) {
      setTimeout(() => {
        this.removeAlert(alertElement);
      }, duration);
    }

    return alertElement;
  }

  /**
   * 移除弹窗
   * @param alertElement 要移除的弹窗元素
   */
  public removeAlert(alertElement: HTMLElement): void {
    // 添加淡出动画效果
    alertElement.style.opacity = '0';
    alertElement.style.transition = 'opacity 0.3s ease-out';

    // 等待动画完成后移除元素
    setTimeout(() => {
      if (alertElement.parentNode) {
        alertElement.parentNode.removeChild(alertElement);
      }

      // 检查是否还有弹窗，如果没有则隐藏遮罩层
      this.checkAndHideContainer();
    }, 300);
  }

  /**
   * 移除所有弹窗
   */
  public removeAllAlerts(): void {
    const container = this.alertsContainer;
    if (container) {
      const alertElements = container.querySelectorAll('.alert-container');
      alertElements.forEach(alert => {
        this.removeAlert(alert as HTMLElement);
      });
    }
  }

  /**
   * 获取或创建弹窗容器
   * @returns 弹窗容器元素
   */
  private getAlertsContainer(): HTMLElement | null {
    if (!this.alertsContainer) {
      this.alertsContainer = document.getElementById('alerts-container');

      // 如果容器不存在，则创建一个
      if (!this.alertsContainer) {
        const newContainer = document.createElement('div');
        newContainer.id = 'alerts-container';
        document.body.appendChild(newContainer);
        this.alertsContainer = newContainer;
      }
    }
    return this.alertsContainer;
  }

  /**
   * 检查是否还有弹窗，如果没有则隐藏容器
   */
  private checkAndHideContainer(): void {
    const container = this.alertsContainer;
    if (container && container.querySelectorAll('.alert-container').length === 0) {
      // 移除遮罩层效果
      container.classList.remove('has-alerts');
    }
  }
}

/**
 * 当DOM加载完成后初始化导航管理器和弹窗管理器
 */
let alertManager: AlertManager;

document.addEventListener('DOMContentLoaded', () => {
  new NavigationManager();
  alertManager = new AlertManager();
});

/**
 * 全局alert函数，可在页面的任何TypeScript代码中调用
 * @param title 弹窗标题 (支持字符串或HTML元素)
 * @param content 弹窗内容 (支持字符串或HTML元素)
 * @param type 弹窗类型 (默认: 'info')
 * @param autoClose 是否自动关闭 (默认: false)
 * @param duration 自动关闭延迟时间(毫秒) (默认: 3000)
 * @returns 创建的弹窗元素
 */
function showAlert(
  title: string | HTMLElement,
  content: string | HTMLElement,
  type: 'info' | 'success' | 'warning' | 'error' = 'info',
  autoClose: boolean = false,
  duration: number = 3000
): HTMLElement {
  // 如果alertManager还未初始化，则延迟执行
  if (!alertManager) {
    setTimeout(() => {
      showAlert(title, content, type, autoClose, duration);
    }, 100);
    // 返回临时元素避免返回undefined
    const tempDiv = document.createElement('div');
    return tempDiv;
  }

  return alertManager.showAlert(title, content, type, autoClose, duration);
}

/**
 * 全局移除弹窗函数
 * @param alertElement 要移除的弹窗元素
 */
function removeAlert(alertElement: HTMLElement): void {
  if (alertManager) {
    alertManager.removeAlert(alertElement);
  }
}

/**
 * 全局移除所有弹窗函数
 */
function removeAllAlerts(): void {
  if (alertManager) {
    alertManager.removeAllAlerts();
  }
}
