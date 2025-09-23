/**
 * 群组分类管理页面的TypeScript实现
 * 负责处理群组分类的显示、添加、编辑、删除和保存功能
 */

/**
 * 群组分类接口定义
 */
interface GroupCategory {
  name: string;
  id: string;
  description: string;
  groups: number[];
}

/**
 * API响应接口定义
 */
interface ApiResponse<T = any> {
  code: number;
  message?: string;
  data?: T;
}

/**
 * 群组分类管理器类
 * 负责处理群组分类页面的所有逻辑
 */
class GroupCategoriesManager {
  private categories: GroupCategory[] = [];
  private currentEditingCategory: GroupCategory | null = null;

  // DOM元素引用
  private categoriesContainer!: HTMLElement;
  private loadingIndicator!: HTMLElement;
  private noCategoriesElement!: HTMLElement;
  private addCategoryBtn!: HTMLElement;
  private saveChangesBtn!: HTMLButtonElement;

  /**
   * 构造函数
   */
  constructor() {
    // 初始化DOM元素引用
    this.initializeDomReferences();

    // 设置事件监听器
    this.setupEventListeners();

    // 加载分类数据
    this.loadCategories();
  }

  /**
   * 初始化DOM元素引用
   */
  private initializeDomReferences(): void {
    this.categoriesContainer = document.getElementById('categories-container')!;
    this.loadingIndicator = document.getElementById('loading-indicator')!;
    this.noCategoriesElement = document.getElementById('no-categories')!;
    this.addCategoryBtn = document.getElementById('add-category-btn')!;
    this.saveChangesBtn = document.getElementById('save-changes-btn')! as HTMLButtonElement;
  }

  /**
   * 设置事件监听器
   */
  private setupEventListeners(): void {
    this.addCategoryBtn.addEventListener('click', () => this.openAddCategoryModal());
    this.saveChangesBtn.addEventListener('click', () => this.saveCategories());
  }

  /**
   * 从API加载群组分类数据
   */
  private async loadCategories(): Promise<void> {
    try {
      this.showLoading();

      const response = await fetch('/api/group-categories/get');
      const result: ApiResponse<GroupCategory[]> = await response.json();

      if (result.code === 0 && result.data) {
        this.categories = result.data;
        this.renderCategories();
      } else {
        showAlert('错误', result.message || '加载分类数据失败', 'error');
        console.error('Failed to load categories:', result);
      }
    } catch (error) {
      showAlert('错误', '加载分类数据时发生网络错误', 'error');
      console.error('Error loading categories:', error);
    } finally {
      this.hideLoading();
    }
  }

  /**
   * 在页面上渲染分类列表
   */
  private renderCategories(): void {
    // 清空容器 - 保存特殊元素的引用并移除其他所有子元素
    const childrenToRemove: Node[] = [];
    
    // 遍历所有子元素
    for (let i = 0; i < this.categoriesContainer.children.length; i++) {
      const child = this.categoriesContainer.children[i];
      
      // 隐藏特殊元素但不移除
      if (child === this.loadingIndicator || child === this.noCategoriesElement) {
        (child as HTMLElement).style.display = 'none';
      } else {
        // 收集需要移除的非特殊元素
        childrenToRemove.push(child);
      }
    }
    
    // 移除所有收集的非特殊元素
    childrenToRemove.forEach(child => {
      this.categoriesContainer.removeChild(child);
    });

    // 检查是否有分类
    if (this.categories.length === 0) {
      this.noCategoriesElement.style.display = 'block';
      return;
    }

    this.noCategoriesElement.style.display = 'none';

    // 创建分类卡片
    this.categories.forEach((category) => {
      const categoryCard = document.createElement('div');
      categoryCard.className = 'category-card';
      categoryCard.innerHTML = `
        <div class="category-header">
          <h3>${category.name}</h3>
          <div class="category-actions">
            <button class="btn btn-sm btn-edit" data-id="${category.id}">
              <i class="fas fa-edit"></i> 编辑
            </button>
            <button class="btn btn-sm btn-danger btn-delete" data-id="${category.id}">
              <i class="fas fa-trash-alt"></i> 删除
            </button>
          </div>
        </div>
        <div class="category-body">
          <p class="category-description">${category.description || '无描述'}</p>
          <div class="category-groups">
            <strong>包含群组：</strong>${category.groups.length} 个群组
          </div>
        </div>
      `;

      this.categoriesContainer.appendChild(categoryCard);

      // 添加编辑和删除按钮的事件监听器
      categoryCard.querySelector(`.btn-edit[data-id="${category.id}"]`)!.addEventListener('click', () => {
        this.openEditCategoryModal(category);
      });

      categoryCard.querySelector(`.btn-delete[data-id="${category.id}"]`)!.addEventListener('click', () => {
        this.deleteCategory(category.id);
      });
    });
  }

  /**
   * 使用showAlert打开添加分类对话框
   */
  private openAddCategoryModal(): void {
    this.currentEditingCategory = null;
    this.openCategoryForm('添加分类', null);
  }

  /**
   * 使用showAlert打开编辑分类对话框
   * @param category 要编辑的分类对象
   */
  private openEditCategoryModal(category: GroupCategory): void {
    this.currentEditingCategory = category;
    this.openCategoryForm('编辑分类', category);
  }

  /**
   * 打开分类表单对话框
   * @param title 对话框标题
   * @param category 分类数据（用于编辑）
   */
  private openCategoryForm(title: string, category: GroupCategory | null): void {
    // 创建表单内容
    const formContainer = document.createElement('div');
    formContainer.className = 'category-form-container';
    formContainer.innerHTML = `
      <div class="form-group">
        <label for="dialog-category-name">分类名称</label>
        <input type="text" id="dialog-category-name" class="form-control" placeholder="请输入分类名称" value="${category ? category.name : ''}">
      </div>
      <div class="form-group">
        <label for="dialog-category-description">分类描述</label>
        <textarea id="dialog-category-description" class="form-control" placeholder="请输入分类描述" rows="3">${category ? category.description || '' : ''}</textarea>
      </div>
      <div class="form-group">
        <label for="dialog-category-groups">群组ID（每行一个）</label>
        <textarea id="dialog-category-groups" class="form-control" placeholder="请输入群组ID，每行一个" rows="5">${category ? category.groups.join('\n') : ''}</textarea>
        <small class="form-text text-muted">输入群组ID，每行一个，用于将群组分配到该分类</small>
      </div>
      <div style="display: flex; gap: 10px; justify-content: flex-end; margin-top: 15px;">
        <button class="btn btn-secondary btn-cancel" data-dismiss="alert">取消</button>
        <button class="btn btn-primary btn-save" data-action="save">保存</button>
      </div>
    `;

    // 显示对话框
    const alertElement = showAlert(title, formContainer, 'info');

    // 为保存和取消按钮添加事件监听
    const saveBtn = formContainer.querySelector('.btn-save')!;
    const cancelBtn = formContainer.querySelector('.btn-cancel')!;
    const categoryNameInput = formContainer.querySelector('#dialog-category-name') as HTMLInputElement;
    const categoryDescriptionInput = formContainer.querySelector('#dialog-category-description') as HTMLTextAreaElement;
    const categoryGroupsInput = formContainer.querySelector('#dialog-category-groups') as HTMLTextAreaElement;

    // 先定义handleSave函数
    const handleSave = () => {
      // 验证表单
      const name = categoryNameInput.value.trim();
      if (!name) {
        showAlert('提示', '请输入分类名称', 'warning', true, 3000);
        return;
      }

      // 处理群组ID
      const groupsText = categoryGroupsInput.value.trim();
      const groups: number[] = [];

      if (groupsText) {
        const groupIds = groupsText.split('\n')
          .map(id => id.trim())
          .filter(id => id);

        for (const id of groupIds) {
          const numId = parseInt(id, 10);
          if (!isNaN(numId)) {
            groups.push(numId);
          }
        }
      }

      // 创建或更新分类
      if (this.currentEditingCategory) {
        // 更新现有分类
        const index = this.categories.findIndex(c => c.id === this.currentEditingCategory!.id);
        if (index !== -1) {
          this.categories[index] = {
            ...this.categories[index],
            name,
            description: categoryDescriptionInput.value.trim(),
            groups
          };
        }
      } else {
        // 创建新分类
        const newCategory: GroupCategory = {
          name,
          id: this.generateUniqueId(name),
          description: categoryDescriptionInput.value.trim(),
          groups
        };
        this.categories.push(newCategory);
      }

      // 调用API保存到服务器
      this.saveCategories().then(() => {
        // 保存成功后关闭对话框并显示成功提示
        removeAlert(alertElement);
      });
      // 重新渲染分类列表
      this.renderCategories();
    };

    const handleCancel = () => {
      removeAlert(alertElement);
      this.currentEditingCategory = null;
    };

    saveBtn.addEventListener('click', handleSave);
    cancelBtn.addEventListener('click', handleCancel);
    
    // 监听alert-save事件
    alertElement.addEventListener('alert-save', (event) => {
      handleSave();
    });

    // 为了防止内存泄漏，确保在弹窗关闭时移除事件监听器
    const cleanup = () => {
      saveBtn.removeEventListener('click', handleSave);
      cancelBtn.removeEventListener('click', handleCancel);
      alertElement.removeEventListener('animationend', cleanup);
    };

    alertElement.addEventListener('animationend', cleanup);
  }

  /**
   * 根据名称生成唯一ID
   * @param name 分类名称
   * @returns 唯一ID字符串
   */
  private generateUniqueId(name: string): string {
    // 生成基础ID（将名称转为小写，替换空格为下划线，移除特殊字符）
    let baseId = name.toLowerCase()
      .replace(/\s+/g, '_')
      .replace(/[^a-z0-9_]/g, '');

    // 如果ID为空，使用默认值
    if (!baseId) {
      baseId = 'category';
    }

    // 检查是否已存在相同ID，如果存在则添加数字后缀
    let uniqueId = baseId;
    let counter = 1;
    while (this.categories.some(c => c.id === uniqueId)) {
      uniqueId = `${baseId}_${counter}`;
      counter++;
    }

    return uniqueId;
  }

  /**
   * 删除分类
   * @param categoryId 要删除的分类ID
   */
  private deleteCategory(categoryId: string): void {
    // 创建自定义确认对话框内容
    const confirmContainer = document.createElement('div');
    confirmContainer.className = 'confirm-actions';
    confirmContainer.innerHTML = `
      <p>确定要删除这个分类吗？</p>
      <div style="display: flex; gap: 10px; justify-content: flex-end; margin-top: 20px;">
        <button class="btn btn-cancel">取消</button>
        <button class="btn btn-danger btn-confirm">删除</button>
      </div>
    `;

    // 显示确认对话框
    const alertElement = showAlert('确认删除', confirmContainer, 'warning');

    // 为确认按钮添加事件监听
    const confirmBtn = confirmContainer.querySelector('.btn-confirm')!;
    const cancelBtn = confirmContainer.querySelector('.btn-cancel')!;

    const handleConfirm = () => {
      const index = this.categories.findIndex(c => c.id === categoryId);
      if (index !== -1) {
        this.categories.splice(index, 1);
        this.renderCategories();
        showAlert('成功', '分类已删除', 'success');
      }
      // 移除事件监听器并关闭弹窗
      removeAlert(alertElement);
    };

    const handleCancel = () => {
      // 移除事件监听器并关闭弹窗
      removeAlert(alertElement);
    };

    confirmBtn.addEventListener('click', handleConfirm);
    cancelBtn.addEventListener('click', handleCancel);

    // 为了防止内存泄漏，确保在弹窗关闭时移除事件监听器
    const cleanup = () => {
      confirmBtn.removeEventListener('click', handleConfirm);
      cancelBtn.removeEventListener('click', handleCancel);
      alertElement.removeEventListener('animationend', cleanup);
    };

    alertElement.addEventListener('animationend', cleanup);
  }

  /**
   * 保存分类配置到服务器
   */
  private async saveCategories(): Promise<void> {
    try {
      this.saveChangesBtn.disabled = true;

      const response = await fetch('/api/group-categories/set', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({ data: this.categories })
      });

      const result: ApiResponse = await response.json();

      if (result.code === 0) {
        showAlert('成功', '群组分类配置已保存', 'success', true, 3000);
      } else {
        showAlert('错误', result.message || '保存配置失败', 'error');
      }
    } catch (error) {
      showAlert('错误', '保存配置时发生网络错误', 'error');
      console.error('Error saving categories:', error);
    } finally {
      this.saveChangesBtn.disabled = false;
    }
  }

  /**
   * 显示加载指示器
   */
  private showLoading(): void {
    this.loadingIndicator.style.display = 'block';
    this.noCategoriesElement.style.display = 'none';
  }

  /**
   * 隐藏加载指示器
   */
  private hideLoading(): void {
    this.loadingIndicator.style.display = 'none';
  }
}

/**
 * 初始化群组分类管理器
 * 在DOM加载完成后执行
 */
function initializeGroupCategoriesManager(): void {
  // 检查是否存在必要的DOM元素
  const categoriesContainer = document.getElementById('categories-container');
  if (!categoriesContainer) {
    console.error('Required DOM elements not found. Retrying in 100ms...');
    setTimeout(initializeGroupCategoriesManager, 100);
    return;
  }

  // 创建管理器实例
  new GroupCategoriesManager();
}

// DOM加载完成后初始化
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', initializeGroupCategoriesManager);
} else {
  // 如果DOM已经加载完成，直接初始化
  initializeGroupCategoriesManager();
}
