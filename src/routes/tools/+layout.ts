import type { Component } from "svelte";
import type { Pathname } from "$app/types";

export type Tools = {
  name: string;
  label: string;
  desc: string;
  path: Pathname;
  icon: Component;
};

export type ToolsCategory = {
  name: string;
  label: string;
  desc: string;
  tools: Tools[];
};

export type ToolsRouter = ToolsCategory[];

const categories = import.meta.glob("./*/category.meta.ts", { eager: true, import: "default" });
const toolsRouter: ToolsRouter = Object.values(categories) as ToolsCategory[];

export const load = async () => {
  return { toolsRouter };
};
