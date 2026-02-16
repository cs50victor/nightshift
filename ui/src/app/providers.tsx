"use client";

import { useRouter } from "next/navigation";
import { RouterProvider } from "react-aria-components";
import Cmd from "@/components/cmd";
import { Toast } from "@/components/ui/toast";
import { ThemeProvider } from "@/providers/theme-provider";

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
