"use client";

import { useRouter } from "next/navigation";
import { RouterProvider } from "react-aria-components";
import { ThemeProvider } from "@/providers/theme-provider";
import { Toast } from "@/components/ui/toast";
import Cmd from "@/components/cmd";

export function Providers({ children }: { children: React.ReactNode }) {
  const router = useRouter();

  return (
    <ThemeProvider>
      <RouterProvider navigate={(path) => router.push(path)}>
        {children}
        <Cmd />
        <Toast position="top-right" />
      </RouterProvider>
    </ThemeProvider>
  );
}
