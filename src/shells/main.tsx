// The main content every shell shows: the navigation stack of pages, or,
// while the session has failed, one page-level state with Retry.

import { View } from "nsh";
import { NavStack } from "nsh/base";
import { t } from "../i18n";
import { ErrorState } from "../components/ui/feedback";
import { nav, type Route } from "../nav";
import { CollectionPage } from "../pages/collection";
import { HomePage } from "../pages/home";
import { SearchPage } from "../pages/search";
import { SettingsPage } from "../pages/settings";
import { health, retry } from "../state/session";

function page(route: Route) {
  switch (route.name) {
    case "home":
      return <HomePage />;
    case "search":
      return <SearchPage />;
    case "collection":
      return <CollectionPage uri={route.uri} />;
    case "settings":
      return <SettingsPage />;
    default:
      return <HomePage />;
  }
}

export function MainStack(props: { class?: string }) {
  return (
    <View class={`min-h-0 flex-1 ${props.class ?? ""}`}>
      <If
        when={health().status !== "failed"}
        else={<ErrorState message={t("main.unavailable")} detail={health().error?.message} onRetry={retry} />}
      >
        <NavStack state={nav} class="flex-1" testID="main">
          {(route) => page(route)}
        </NavStack>
      </If>
    </View>
  );
}
