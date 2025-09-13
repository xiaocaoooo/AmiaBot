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
    // 初始化时根据当前URL哈希值导航到对应页面
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
        this.navigateTo(targetId);
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
    // 从URL中获取哈希值（不包含#符号）
    const hash = window.location.hash.substring(1) || 'base-info';
    this.navigateTo(hash, false);
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
        link.getAttribute('href') === `#${targetId}` || link.getAttribute('data-id') === targetId
      )?.textContent;
      this.pageNameElement.textContent = linkText || '未知页面';
    }

    // 如果需要更新历史记录，则添加到浏览器历史
    if (updateHistory) {
      history.pushState(null, '', `#${targetId}`);
    }

    // 高亮当前活动的导航链接
    this.navLinks.forEach(link => {
      const linkTargetId = link.getAttribute('href')?.substring(1) || link.getAttribute('data-id') || '';
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
 * 当DOM加载完成后初始化导航管理器
 */
document.addEventListener('DOMContentLoaded', () => {
  new NavigationManager();
});
