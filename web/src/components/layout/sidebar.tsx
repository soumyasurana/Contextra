'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import {
  LayoutDashboard, FileText, FolderArchive, MessageSquare, History,
  GitMerge, Code2, BarChart3, Settings, Cpu,
} from 'lucide-react';

export const navItems = [
  { name: 'Dashboard', icon: LayoutDashboard, href: '/dashboard' },
  { name: 'Documents', icon: FileText, href: '/documents' },
  { name: 'Collections', icon: FolderArchive, href: '/collections' },
  { name: 'Playground Chat', icon: MessageSquare, href: '/chat' },
  { name: 'Conversations', icon: History, href: '/conversations' },
  { name: 'Retrieval Explorer', icon: GitMerge, href: '/retrieval' },
  { name: 'Prompt Studio', icon: Code2, href: '/prompts' },
  { name: 'Evaluation', icon: BarChart3, href: '/evaluation' },
  { name: 'Settings', icon: Settings, href: '/settings' },
];

export function Sidebar() {
  const pathname = usePathname();

  return (
    <aside className="glass-panel shrink-0 border-b border-white/10 bg-[#0c0e17] px-3 pt-3 lg:fixed lg:inset-y-0 lg:left-0 lg:z-40 lg:w-64 lg:overflow-y-auto lg:border-b-0 lg:border-r lg:p-4">
      <Link href="/" className="flex w-fit items-center gap-3 px-2 py-1 text-lg font-bold text-white">
        <span className="flex size-8 items-center justify-center rounded-lg bg-linear-to-tr from-indigo-600 to-purple-600">
          <Cpu className="size-4" />
        </span>
        Contextra
      </Link>
      <nav aria-label="Workspace" className="mt-2 flex gap-1 overflow-x-auto px-1 py-2 lg:mt-6 lg:flex-col lg:overflow-visible">
        {navItems.map(({ name, href, icon: Icon }) => {
          const current = pathname === href;
          return (
            <Link
              key={href}
              href={href}
              aria-current={current ? 'page' : undefined}
              className={`flex shrink-0 items-center gap-2 rounded-xl border px-3 py-2 text-sm font-medium whitespace-nowrap transition-colors ${current ? 'border-indigo-500/40 bg-indigo-600/20 text-white' : 'border-transparent text-zinc-400 hover:bg-white/5 hover:text-zinc-200'}`}
            >
              <Icon className={`size-4 ${current ? 'text-indigo-400' : 'text-zinc-500'}`} />
              {name}
            </Link>
          );
        })}
      </nav>
    </aside>
  );
}
