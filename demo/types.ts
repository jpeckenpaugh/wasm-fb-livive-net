// Domain types for the Orbit checkout and order fulfillment API.

export type OrderStatus = 'draft' | 'reserved' | 'paid' | 'shipped';

export interface LineItem {
  readonly sku: string;
  readonly title: string;
  readonly unitCents: number;
  readonly quantity: number;
}

export interface OrderSummary {
  readonly id: string;
  readonly status: OrderStatus;
  readonly items: readonly LineItem[];
  readonly discountCents: number;
}

export function computeOrderTotalCents(order: OrderSummary): number {
  let subtotal = 0;
  for (const item of order.items) {
    subtotal += item.unitCents * item.quantity;
  }
  const total = subtotal - order.discountCents;
  return total > 0 ? total : 0;
}
