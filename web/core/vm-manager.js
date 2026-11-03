/* VmManager: Slot-Aware Virtual Machine Lifecycle & Identity Coordinator */

let nextId = 1;

export class VmManager {
  constructor(deps = {}) {
    this.deps = deps;
    this.guests = [];
    this.activeId = null;
    this.splitView = false;
  }

  getGuests() {
    return this.guests;
  }

  getActiveGuest() {
    return this.guests.find((g) => g.id === this.activeId) || this.guests[0] || null;
  }

  getById(id) {
    return this.guests.find((g) => g.id === id) || null;
  }

  getBySeat(seat) {
    return this.guests.find((g) => g.mpSeat === seat) || null;
  }

  /* Stable deterministic slot resolution for AutoPilot / 2P matching */
  getByTarget(targetVm) {
    if (!targetVm || targetVm === 'active') return this.getActiveGuest();
    if (typeof targetVm === 'number') {
      return this.guests.find((g) => g.id === targetVm) || this.guests[targetVm] || null;
    }
    const t = String(targetVm).toLowerCase();
    if (t === 'p1' || t === 'seat1' || t === 'seat-1' || t === 'vm-01' || t === 'vm1') {
      return this.guests.find((g) => g.mpSeat === 1) || this.guests[0] || null;
    }
    if (t === 'p2' || t === 'seat2' || t === 'seat-2' || t === 'vm-02' || t === 'vm2') {
      return this.guests.find((g) => g.mpSeat === 2) || this.guests[1] || null;
    }
    return this.guests.find((g) => g.name === targetVm || String(g.id) === t) || null;
  }

  /* Reset VM ID numbering when all guests are destroyed */
  resetIdSequence() {
    if (this.guests.length === 0) {
      nextId = 1;
    }
  }

  nextVmId() {
    return nextId++;
  }

  addGuest(vm) {
    this.guests.push(vm);
    if (!this.activeId) {
      this.activeId = vm.id;
    }
    return vm;
  }

  removeGuest(vmOrId) {
    const id = typeof vmOrId === 'object' ? vmOrId.id : vmOrId;
    const idx = this.guests.findIndex((g) => g.id === id);
    if (idx < 0) return null;

    const [removed] = this.guests.splice(idx, 1);
    removed.disposed = true;

    if (this.activeId === id) {
      const next = this.guests[Math.min(idx, this.guests.length - 1)];
      this.activeId = next ? next.id : null;
    }

    if (this.guests.length === 0) {
      nextId = 1;
    }
    return removed;
  }
}
