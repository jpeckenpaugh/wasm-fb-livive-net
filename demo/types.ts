// typed variant of the demo module
interface Item {
  name: string;
  slots: number;
}

export function total(items: Item[]): number {
  return items.reduce((sum, item) => sum + item.slots, 0);
}

const apps: Item[] = [{ name: "clock", slots: 1 }];
console.log(total(apps));
