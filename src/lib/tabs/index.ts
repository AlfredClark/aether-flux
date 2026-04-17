import type { Pathname } from "$app/types";

export type Tab = {
  name: string;
  label: string;
  path: Pathname;
  home: boolean;
};

type Tabs = Tab[];

function sortByNameOrder<T extends { name: string }>(items: T[], order: string[]): T[] {
  const orderMap = new Map(order.map((name, idx) => [name, idx]));

  return [...items].sort((a, b) => {
    const indexA = orderMap.has(a.name) ? orderMap.get(a.name)! : Infinity;
    const indexB = orderMap.has(b.name) ? orderMap.get(b.name)! : Infinity;
    return indexA - indexB;
  });
}

const tabsOrder = ["home", "tools", "settings", "about"];

const meta = import.meta.glob("../../routes/**/tabs.meta.ts", { eager: true, import: "default" });

export function getTabs(): Tabs {
  return sortByNameOrder(Object.values(meta) as Tabs, tabsOrder);
}
