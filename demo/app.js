// TaskBoard client module: filters, renders, and toggles project tasks.
const API_BASE = '/api/v1/projects';

export class TaskBoard {
  constructor(rootEl, projectId) {
    this.root = rootEl;
    this.projectId = projectId;
    this.items = [];
    this.filter = 'open';
  }

  async refresh() {
    const res = await fetch(`${API_BASE}/${this.projectId}/tasks`);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const payload = await res.json();
    this.items = payload.tasks || [];
    this.render();
    return this.items.length;
  }

  visibleItems() {
    if (this.filter === 'all') return this.items;
    const wantDone = this.filter === 'done';
    return this.items.filter((task) => Boolean(task.completed) === wantDone);
  }

  toggleTask(taskId) {
    for (const item of this.items) {
      if (item.id === taskId) {
        item.completed = !item.completed;
        break;
      }
    }
    this.render();
  }

  render() {
    const rows = this.visibleItems();
    this.root.dataset.count = String(rows.length);
  }
}
