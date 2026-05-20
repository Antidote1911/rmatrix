#!/usr/bin/env bash
# =============================================================================
#  git.sh — Assistant Git
#  Usage rapide  : ./git.sh "mon message de commit"
#  Menu interactif : ./git.sh
# =============================================================================

set -euo pipefail
IFS=$'\n\t'

# ── Couleurs ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

info()    { echo -e "${CYAN}ℹ ${RESET}$*"; }
success() { echo -e "${GREEN}✔ ${RESET}$*"; }
warn()    { echo -e "${YELLOW}⚠ ${RESET}$*"; }
error()   { echo -e "${RED}✖ ${RESET}$*" >&2; }
title()   { echo -e "\n${BOLD}${BLUE}━━━ $* ━━━${RESET}\n"; }
sep()     { echo -e "${BLUE}────────────────────────────────────────${RESET}"; }

confirm() {
  read -rp "$(echo -e "${YELLOW}? ${RESET}${1:-Confirmer ?} [o/N] ")" ans
  [[ "${ans,,}" =~ ^(o|oui|y|yes)$ ]]
}

require_git() {
  if ! git rev-parse --git-dir &>/dev/null; then
    error "Ce dossier n'est pas un dépôt Git."
    exit 1
  fi
}

# ── Gestion des dossiers à ignorer ───────────────────────────────────────────
# Liste des patterns à toujours ignorer (personnalisable)
IGNORE_PATTERNS=("target/" "node_modules/" ".env" "dist/" "__pycache__/")

setup_gitignore() {
  [[ ! -f .gitignore ]] && touch .gitignore

  for pattern in "${IGNORE_PATTERNS[@]}"; do
    if ! grep -qxF "$pattern" .gitignore 2>/dev/null; then
      printf '%s\n' "$pattern" >> .gitignore
      info "Ajouté au .gitignore : $pattern"
    fi
  done
}

# Retire du suivi Git les fichiers qui doivent être ignorés mais sont trackés
untrack_ignored() {
  for pattern in "${IGNORE_PATTERNS[@]}"; do
    local dir="${pattern%/}"
    if git ls-files -- "$dir" 2>/dev/null | grep -q .; then
      warn "Retrait de l'index : ${pattern}"
      git rm -r --cached "$dir" 2>/dev/null || true
    fi
  done
}

# ── Stage intelligent (exclut les patterns ignorés) ───────────────────────────
smart_add() {
  set +e
  # Construit les exclusions pour git add
  local excludes=()
  for p in "${IGNORE_PATTERNS[@]}"; do
    excludes+=(":!${p}" ":!${p}**")
  done

  if git add . -- "${excludes[@]}" 2>/dev/null; then
    success "Fichiers stagés (dossiers ignorés exclus)."
  else
    # Fallback : add -A puis reset les ignorés
    git add -A
    for p in "${IGNORE_PATTERNS[@]}"; do
      git reset -- "$p" "${p}**" >/dev/null 2>&1 || true
    done
    success "Fichiers stagés (fallback)."
  fi
  set -e
}

# ── Commit + Push (cœur du script) ───────────────────────────────────────────
commit_and_push() {
  local msg="${1:-}"
  local branch
  branch=$(git rev-parse --abbrev-ref HEAD)

  info "Branche : ${GREEN}${branch}${RESET}"

  # .gitignore + nettoyage index
  setup_gitignore
  untrack_ignored

  # Vérifier s'il y a quelque chose à committer
  smart_add

  if git diff --cached --quiet; then
    warn "Rien à committer — synchronisation uniquement."
    git pull --rebase origin "$branch"
    git push origin "$branch"
    success "Dépôt à jour."
    return
  fi

  # Message de commit
  if [[ -z "$msg" ]]; then
    read -rp "$(echo -e "${YELLOW}? ${RESET}Message de commit : ")" msg
    [[ -z "$msg" ]] && msg="Update"
  fi

  git commit -m "$msg"
  success "Commit : \"$msg\""

  info "Pull --rebase depuis origin/${branch}..."
  git pull --rebase origin "$branch"

  info "Push vers origin/${branch}..."
  git push origin "$branch"

  success "Tout est pushé ✓"
}

# ── Actions du menu ───────────────────────────────────────────────────────────
do_status() {
  title "État du dépôt"
  local branch
  branch=$(git symbolic-ref --short HEAD 2>/dev/null || echo "HEAD détachée")
  local remote
  remote=$(git remote get-url origin 2>/dev/null || echo "(pas de remote)")
  echo -e "  ${BOLD}Branche :${RESET} ${GREEN}${branch}${RESET}"
  echo -e "  ${BOLD}Remote  :${RESET} ${remote}"
  echo ""
  git status -sb
}

do_tag() {
  title "Créer un tag"
  info "Tags existants :"
  git tag --sort=-version:refname | head -10 || echo "  (aucun)"
  echo ""
  read -rp "$(echo -e "${YELLOW}? ${RESET}Nom du tag (ex: v1.2.3) : ")" tag
  [[ -z "$tag" ]] && { error "Nom vide — annulé."; return; }
  if git rev-parse "$tag" &>/dev/null 2>&1; then
    error "Le tag '${tag}' existe déjà."
    return
  fi
  read -rp "$(echo -e "${YELLOW}? ${RESET}Message du tag (vide = tag léger) : ")" tmsg
  if [[ -n "$tmsg" ]]; then git tag -a "$tag" -m "$tmsg"; else git tag "$tag"; fi
  success "Tag '${tag}' créé."
  if confirm "Pusher ce tag sur origin ?"; then
    git push origin "$tag"
    success "Tag '${tag}' pushé."
  fi
}

do_pull() {
  title "Pull"
  local branch
  branch=$(git symbolic-ref --short HEAD)
  echo -e "  1) pull (merge)\n  2) pull --rebase\n"
  read -rp "$(echo -e "${YELLOW}? ${RESET}Choix [2] : ")" c
  case "${c:-2}" in
    1) git pull origin "$branch" && success "Pull effectué." ;;
    *) git pull --rebase origin "$branch" && success "Pull --rebase effectué." ;;
  esac
}

do_branch() {
  title "Branches"
  echo -e "  1) Lister\n  2) Créer\n  3) Changer\n  4) Supprimer\n  5) Retour\n"
  read -rp "$(echo -e "${YELLOW}? ${RESET}Choix : ")" c
  case "$c" in
    1) git branch -vva ;;
    2)
      read -rp "$(echo -e "${YELLOW}? ${RESET}Nom de la nouvelle branche : ")" nb
      [[ -z "$nb" ]] && { error "Nom vide."; return; }
      git checkout -b "$nb" && success "Branche '${nb}' créée."
      if confirm "Pusher et créer l'upstream ?"; then
        git push --set-upstream origin "$nb"
      fi ;;
    3)
      git branch -a
      read -rp "$(echo -e "${YELLOW}? ${RESET}Nom de la branche : ")" sb
      git checkout "$sb" && success "Basculé sur '${sb}'." ;;
    4)
      git branch -vva
      read -rp "$(echo -e "${YELLOW}? ${RESET}Branche à supprimer : ")" db
      confirm "Supprimer '${db}' localement ?" && git branch -d "$db"
      confirm "Supprimer '${db}' sur origin aussi ?" && git push origin --delete "$db" ;;
    *) return ;;
  esac
}

do_stash() {
  title "Stash"
  echo -e "  1) Sauvegarder\n  2) Lister\n  3) Restaurer (pop)\n  4) Retour\n"
  read -rp "$(echo -e "${YELLOW}? ${RESET}Choix : ")" c
  case "$c" in
    1)
      read -rp "$(echo -e "${YELLOW}? ${RESET}Message (optionnel) : ")" sm
      if [[ -n "$sm" ]]; then git stash push -m "$sm"; else git stash push; fi
      success "Stash sauvegardé." ;;
    2) git stash list ;;
    3) git stash pop && success "Stash restauré." ;;
    *) return ;;
  esac
}

do_log() {
  title "Historique"
  git log --oneline --graph --decorate --color --all | head -30
}

do_init() {
  title "Initialiser un dépôt"
  if git rev-parse --git-dir &>/dev/null 2>&1; then
    warn "Un dépôt Git existe déjà ici."; return
  fi
  git init && success "Dépôt initialisé."
  read -rp "$(echo -e "${YELLOW}? ${RESET}URL du remote origin (vide pour ignorer) : ")" rurl
  if [[ -n "$rurl" ]]; then
    git remote add origin "$rurl" && success "Remote 'origin' configuré."
  fi
}

# ── Menu interactif ───────────────────────────────────────────────────────────
main_menu() {
  while true; do
    echo ""
    if git rev-parse --git-dir &>/dev/null 2>&1; then
      show_branch_banner
    else
      echo -e "${BOLD}${BLUE}╔══════════════════════════════════════╗${RESET}"
      echo -e "${BOLD}${BLUE}║        🚀  Assistant Git             ║${RESET}"
      echo -e "${BOLD}${BLUE}╚══════════════════════════════════════╝${RESET}"
      echo ""
    fi

    echo -e "  ${BOLD}1${RESET}  Commit + Push            ${CYAN}← équivalent à ./git.sh \"msg\"${RESET}"
    echo -e "  ${BOLD}2${RESET}  Créer & pusher un tag"
    echo -e "  ${BOLD}3${RESET}  Pull / Synchroniser"
    echo -e "  ${BOLD}4${RESET}  Branches"
    echo -e "  ${BOLD}5${RESET}  Stash"
    echo -e "  ${BOLD}6${RESET}  Historique (log)"
    echo -e "  ${BOLD}7${RESET}  Status"
    echo -e "  ${BOLD}0${RESET}  Initialiser un nouveau dépôt"
    echo -e "  ${BOLD}q${RESET}  Quitter"
    echo ""
    sep

    read -rp "$(echo -e "${YELLOW}➜ ${RESET}Choix : ")" choice

    case "$choice" in
      1|2|3|4|5|6|7) require_git ;;
    esac

    case "$choice" in
      1) commit_and_push "" ;;
      2) do_tag ;;
      3) do_pull ;;
      4) do_branch ;;
      5) do_stash ;;
      6) do_log ;;
      7) do_status ;;
      0) do_init ;;
      q|Q) echo -e "\n${GREEN}À bientôt !${RESET}\n"; exit 0 ;;
      *) warn "Choix invalide." ;;
    esac
  done
}

# ── Bandeau branche ───────────────────────────────────────────────────────────
show_branch_banner() {
  local branch
  branch=$(git symbolic-ref --short HEAD 2>/dev/null || echo "HEAD détachée")
  local dirty=""
  git diff --quiet 2>/dev/null || dirty=" ${RED}●${RESET}"
  local remote
  remote=$(git remote get-url origin 2>/dev/null | sed 's/.*github.com[:/]//' | sed 's/\.git$//' || echo "")

  echo ""
  echo -e "${BOLD}${BLUE}╔══════════════════════════════════════╗${RESET}"
  printf "${BOLD}${BLUE}║${RESET}  🌿 %-34s${BOLD}${BLUE}║${RESET}\n" "${branch}${dirty@P}"
  [[ -n "$remote" ]] && \
  printf "${BOLD}${BLUE}║${RESET}  📦 %-34s${BOLD}${BLUE}║${RESET}\n" "$remote"
  echo -e "${BOLD}${BLUE}╚══════════════════════════════════════╝${RESET}"
  echo ""
}

# ── Point d'entrée ────────────────────────────────────────────────────────────
# Avec argument → commit+push direct (comme ton ancien script)
# Sans argument  → menu interactif

if [[ $# -gt 0 ]]; then
  require_git
  show_branch_banner
  commit_and_push "$*"
else
  main_menu
fi
