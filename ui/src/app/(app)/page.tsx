"use client";
import { useEffect } from "react";
import EmptyState from "@/components/empty-state";
import { useBreadcrumb } from "@/contexts/breadcrumb-context";

export default function AppIndex() {
  const { setPageTitle } = useBreadcrumb();

  useEffect(() => {
    setPageTitle(null);
    return () => setPageTitle(null);
  }, [setPageTitle]);

  return <EmptyState />;
}
